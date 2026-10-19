"""StreamDen spike S-3: minimal Python script worker (stdlib only, throwaway).

Protocol: JSON-RPC 2.0 over stdin/stdout, LSP-style framing
("Content-Length: N\\r\\n\\r\\n" + N bytes of UTF-8 JSON). Bidirectional:
  host -> worker  request       execute {code, args}
  host -> worker  notification  $/cancelRequest {id}      (soft timeout, portable)
  host -> worker  SIGINT signal                           (soft timeout, Unix alternative)
  worker -> host  notification  ready {pid, version}
  worker -> host  request       globals.get {name} / globals.set {name, value} / log {message}
  worker -> host  notification  log {message}             (captured print())

Threads: main thread runs user code only. A reader thread owns the protocol input
(so the host can always write, even while a script is busy) and a writer thread
owns the protocol output (so a KeyboardInterrupt can never tear a frame in half).
"""

import builtins
import faulthandler
import itertools
import json
import os
import queue
import signal
import sys
import threading
import _thread

# --- 1. Take the real stdin/stdout for the protocol; make fd 0/1 harmless. ------------
_proto_in = os.fdopen(os.dup(0), "rb")
_proto_out = os.fdopen(os.dup(1), "wb")
_devnull = os.open(os.devnull, os.O_RDONLY)
os.dup2(_devnull, 0)  # input() gets EOF instead of eating protocol bytes
os.dup2(2, 1)  # print/os.write(1) from C code lands on stderr, not in the protocol
sys.stdin = open(os.devnull, "r")

# --- 2. Robustness knobs. -------------------------------------------------------------
faulthandler.enable(file=sys.stderr)  # segfault -> Python traceback on stderr for the host
# A parent that ignores SIGINT (nohup, some launchers) makes Python keep SIG_IGN, and the
# soft timeout would silently never fire. Install the default handler explicitly.
signal.signal(signal.SIGINT, signal.default_int_handler)

if os.environ.get("DEN_WORKER_NO_COREDUMP") == "1" and sys.platform == "linux":
    # A crashing user script must not trigger systemd-coredump (~100-200 ms, journal spam).
    # RLIMIT_CORE=0 is not enough when core_pattern pipes to a helper; non-dumpable is.
    # Trade-off: debuggers/py-spy cannot attach to a non-dumpable process.
    import ctypes

    ctypes.CDLL(None).prctl(4, 0, 0, 0, 0)  # PR_SET_DUMPABLE = 4

_max_mem_mb = int(os.environ.get("DEN_WORKER_MAX_MEM_MB", "0"))
if _max_mem_mb > 0:
    try:
        import resource

        _lim = _max_mem_mb * 1024 * 1024
        resource.setrlimit(resource.RLIMIT_AS, (_lim, _lim))
    except (ImportError, ValueError, OSError) as e:  # Windows: no resource module
        print(f"rlimit not applied: {e}", file=sys.stderr)

_inbox = queue.SimpleQueue()  # host requests (execute)
_responses = queue.SimpleQueue()  # responses to our api calls
_outbox = queue.SimpleQueue()  # encoded frames to write
_EOF = object()
_main_ident = threading.main_thread().ident
_current_exec = None  # id of the execute request in flight (read by reader thread)
_ids = itertools.count(1)


def _block_sigint_in_this_thread():
    # Only the main thread may receive SIGINT. Then a process-directed SIGINT from the host
    # (kill(pid, SIGINT)) always interrupts the main thread's blocking syscall, too.
    if hasattr(signal, "pthread_sigmask"):
        signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGINT})


def _reader():
    _block_sigint_in_this_thread()
    while True:
        length = None
        while True:
            line = _proto_in.readline()
            if not line:
                os._exit(0)  # host closed the pipe (or died): never outlive it
            line = line.strip()
            if not line:
                break
            k, _, v = line.partition(b":")
            if k.strip().lower() == b"content-length":
                length = int(v)
        body = _proto_in.read(length)
        if len(body) < length:
            os._exit(0)
        msg = json.loads(body)
        if "method" in msg:
            if msg["method"] == "$/cancelRequest":
                if msg.get("params", {}).get("id") == _current_exec:
                    _interrupt_main()
            elif "id" in msg:
                _inbox.put(msg)
        else:
            _responses.put(msg)


def _interrupt_main():
    # pthread_kill delivers a real signal to the main thread, which also wakes blocking
    # syscalls (time.sleep, socket reads). interrupt_main() only sets a flag that is
    # checked between bytecodes; it is the fallback where pthread_kill does not exist.
    if hasattr(signal, "pthread_kill"):
        signal.pthread_kill(_main_ident, signal.SIGINT)
    else:
        _thread.interrupt_main()


def _writer():
    _block_sigint_in_this_thread()
    while True:
        frame = _outbox.get()
        _proto_out.write(frame)
        _proto_out.flush()


def _send(obj):
    body = json.dumps(obj, separators=(",", ":"), default=str).encode()
    _outbox.put(b"Content-Length: %d\r\n\r\n%s" % (len(body), body))


class ApiError(Exception):
    pass


def _call(method, params):
    rid = "w%d" % next(_ids)
    _send({"jsonrpc": "2.0", "id": rid, "method": method, "params": params})
    while True:
        msg = _responses.get()
        if msg.get("id") == rid:
            break  # anything else is a stale reply to a call that was interrupted
    if "error" in msg:
        raise ApiError(msg["error"].get("message", "api error"))
    return msg.get("result")


class _Globals:
    def get(self, name, default=None):
        v = _call("globals.get", {"name": name})
        return default if v is None else v

    def set(self, name, value, persist=False):
        _call("globals.set", {"name": name, "value": value, "persist": persist})


class Api:
    def __init__(self, state):
        self._state = state
        self.globals = _Globals()

    def log(self, *parts):
        _call("log", {"message": " ".join(str(p) for p in parts)})

    def set_arg(self, name, value):
        self._state["setArgs"][name] = value


class _PrintCapture:
    """sys.stdout replacement: complete lines become `log` notifications (no round trip)."""

    def __init__(self):
        self._buf = ""

    def write(self, s):
        self._buf += s
        while "\n" in self._buf:
            line, self._buf = self._buf.split("\n", 1)
            _send({"jsonrpc": "2.0", "method": "log", "params": {"message": line}})
        return len(s)

    def flush(self):
        if self._buf:
            _send({"jsonrpc": "2.0", "method": "log", "params": {"message": self._buf}})
            self._buf = ""


_code_cache = {}


def _execute(params):
    code = params["code"]
    co = _code_cache.get(code)
    if co is None:
        co = compile(code, "<script>", "exec")
        if len(_code_cache) > 256:
            _code_cache.clear()
        _code_cache[code] = co
    args = params.get("args") or {}
    state = {"setArgs": {}}
    api = Api(state)
    ns = {"__name__": "__den_script__", "__builtins__": builtins, "args": args, "api": api}
    # Fresh capture per execution, flushed on every exit path (error and cancel included) so
    # a partial line is sent before this run's reply and never leaks into the next run.
    cap = sys.stdout = _PrintCapture()
    try:
        exec(co, ns)  # fresh module namespace per execution
        ret = None
        fn = ns.get("execute")
        if callable(fn):
            ret = fn(args, api)
    finally:
        cap.flush()
    return {"ok": ret is not False, "setArgs": state["setArgs"]}


def main():
    global _current_exec
    threading.Thread(target=_reader, name="den-reader", daemon=True).start()
    threading.Thread(target=_writer, name="den-writer", daemon=True).start()
    _send({"jsonrpc": "2.0", "method": "ready",
           "params": {"pid": os.getpid(), "version": sys.version.split()[0]}})
    while True:
        reply = None
        try:
            msg = _inbox.get()
            if msg.get("method") != "execute":
                _send({"jsonrpc": "2.0", "id": msg["id"],
                       "error": {"code": -32601, "message": "method not found"}})
                continue
            _current_exec = msg["id"]
            try:
                result = _execute(msg["params"])
                reply = {"jsonrpc": "2.0", "id": msg["id"], "result": result}
            except KeyboardInterrupt:
                reply = {"jsonrpc": "2.0", "id": msg["id"],
                         "error": {"code": -32800, "message": "cancelled (soft timeout)"}}
            except BaseException as e:  # noqa: BLE001 - report everything incl. SystemExit
                reply = {"jsonrpc": "2.0", "id": msg["id"],
                         "error": {"code": -32000, "message": f"{type(e).__name__}: {e}"}}
            finally:
                _current_exec = None
            _send(reply)
            reply = None
        except KeyboardInterrupt:
            # A cancel signal raced with completion. Never lose the reply (a duplicate is
            # harmless: the host ignores responses for unknown ids).
            if reply is not None:
                _send(reply)


if __name__ == "__main__":
    main()
