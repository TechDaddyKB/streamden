import { useRef, useState, type KeyboardEvent } from 'react';
import { actions, categories, load, save, score, type Action } from './model';
import css from './Picker.module.css';
type Row = { id: string; label: string; category: string; action?: Action };
type View = 'All actions' | 'Favourites' | 'Recent';
export function Picker() {
  const [open, setOpen] = useState(true), [query, setQuery] = useState('');
  const [view, setView] = useState<View>('All actions');
  const [expanded, setExpanded] = useState<string[]>(['Core']);
  const [active, setActive] = useState('category-Core');
  const [favourites, setFavourites] = useState(() => load('b3-favourites'));
  const [recent, setRecent] = useState(() => load('b3-recent').slice(0, 10));
  const [selected, setSelected] = useState<Action>();
  const nodes = useRef(new Map<string, HTMLLIElement>()), opener = useRef<HTMLButtonElement>(null);
  const typeahead = useRef({ text: '', time: 0 });
  const matches = actions.filter(a => (view === 'All actions' || (view === 'Favourites' ? favourites : recent).includes(a.id)) && Number.isFinite(score(`${a.name} ${a.category}`, query)))
    .sort((a, b) => query.trim() ? score(`${a.name} ${a.category}`, query) - score(`${b.name} ${b.category}`, query) : view === 'Recent' ? recent.indexOf(a.id) - recent.indexOf(b.id) : 0);
  const isExpanded = (category: string) => !!query.trim() || expanded.includes(category);
  const groups = categories.map(category => ({ category, items: matches.filter(a => a.category === category) })).filter(g => g.items.length);
  const rows: Row[] = groups.flatMap(g => [{ id: `category-${g.category}`, label: g.category, category: g.category }, ...(isExpanded(g.category) ? g.items.map(action => ({ id: action.id, label: action.name, category: g.category, action })) : [])]);
  const current = rows.find(r => r.id === active) ?? rows[0];
  const candidate = current?.action;
  function focus(row?: Row) { if (row) { setActive(row.id); nodes.current.get(row.id)?.focus(); } }
  function toggle(category: string) { setExpanded(old => old.includes(category) ? old.filter(c => c !== category) : [...old, category]); }
  function close() { setOpen(false); opener.current?.focus(); }
  function choose(action: Action) {
    const next = [action.id, ...recent.filter(id => id !== action.id)].slice(0, 10);
    setRecent(next); save('b3-recent', next); setSelected(action); close();
  }
  function favourite() {
    if (!candidate) return;
    const next = favourites.includes(candidate.id) ? favourites.filter(id => id !== candidate.id) : [...favourites, candidate.id];
    setFavourites(next); save('b3-favourites', next);
  }
  function navigate(event: KeyboardEvent<HTMLUListElement>) {
    if (!current || event.altKey || event.ctrlKey || event.metaKey) return;
    const index = rows.indexOf(current), key = event.key;
    if (['ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight', 'Home', 'End', 'Enter', ' '].includes(key)) event.preventDefault();
    if (key === 'ArrowDown') focus(rows[Math.min(index + 1, rows.length - 1)]);
    else if (key === 'ArrowUp') focus(rows[Math.max(index - 1, 0)]);
    else if (key === 'Home') focus(rows[0]);
    else if (key === 'End') focus(rows.at(-1));
    else if (key === 'ArrowRight' && !current.action) { if (!isExpanded(current.category)) toggle(current.category); else focus(rows[index + 1]); }
    else if (key === 'ArrowLeft') { if (current.action) focus(rows.find(r => r.id === `category-${current.category}`)); else if (isExpanded(current.category) && !query.trim()) toggle(current.category); }
    else if (key === 'Enter' || key === ' ') { if (current.action) choose(current.action); else if (!query.trim()) toggle(current.category); }
    else if (key.length === 1) {
      const now = Date.now(), previous = typeahead.current;
      const text = now - previous.time < 700 ? previous.text + key.toLowerCase() : key.toLowerCase();
      typeahead.current = { text, time: now };
      const prefix = [...text].every(c => c === text[0]) ? text[0] : text;
      focus([...rows.slice(index + 1), ...rows.slice(0, index + 1)].find(r => r.label.toLowerCase().startsWith(prefix)));
    }
  }
  return <main className={css.app}>
    <header className={css.brand}><span className={css.logo}>S</span><strong>StreamDen</strong><span className={css.badge}>ACTION STUDIO</span></header>
    <div className={css.workspace}><p className={css.eyebrow}>AUTOMATIONS / STREAM STARTING</p><h1>Build your next moment.</h1><p>Connect small actions. Make the whole stream yours.</p>
      <button ref={opener} onClick={() => setOpen(true)}>＋ Add sub-action</button><p role="status">{selected ? `Added: ${selected.name} (${selected.category})` : 'Your action sequence is ready for a new step.'}</p>
    </div>
    {open && <section className={css.picker} aria-label="Add sub-action" onKeyDown={e => { if (e.key === 'Escape') { e.stopPropagation(); close(); } }}>
      <header className={css.heading}><div><p className={css.eyebrow}>ACTION LIBRARY</p><h2>Add sub-action</h2></div><button aria-label="Close picker" onClick={close}>✕</button></header>
      <div className={css.search}><label htmlFor="search">Search actions</label><input autoFocus id="search" placeholder="Search by name or category…" value={query} onChange={e => { setQuery(e.target.value); setActive(''); }} onKeyDown={e => { if (e.key === 'ArrowDown') { e.preventDefault(); focus(rows[0]); } }} /><span>{matches.length} of {actions.length} actions</span></div>
      <nav className={css.tabs} aria-label="Library views">{(['All actions', 'Favourites', 'Recent'] as View[]).map(tab => <button key={tab} aria-pressed={view === tab} onClick={() => { setView(tab); setActive(''); }}>{tab}{tab === 'Favourites' ? ` · ${favourites.length}` : tab === 'Recent' ? ` · ${recent.length}` : ''}</button>)}</nav>
      <div className={css.content}>
        <div className={css.treePane}>{!rows.length && <p className={css.empty}>No actions found. Try another search or library view.</p>}
          <ul role="tree" aria-label="Sub-actions" className={css.tree} onKeyDown={navigate}>
            {groups.map(({ category, items }) => <li key={category} role="treeitem" aria-label={category} aria-expanded={isExpanded(category)} tabIndex={current?.id === `category-${category}` ? 0 : -1} ref={el => { if (el) nodes.current.set(`category-${category}`, el); else nodes.current.delete(`category-${category}`); }} onFocus={e => { if (e.target === e.currentTarget) setActive(`category-${category}`); }}>
              <div className={css.category} onClick={() => { focus(rows.find(r => r.id === `category-${category}`)); if (!query.trim()) toggle(category); }}><span>{isExpanded(category) ? '▾' : '▸'}</span><strong>{category}</strong><small>{items.length}</small></div>
              {isExpanded(category) && <ul role="group">{items.map(action => <li key={action.id} role="treeitem" aria-label={action.name} aria-selected={candidate?.id === action.id} tabIndex={current?.id === action.id ? 0 : -1} ref={el => { if (el) nodes.current.set(action.id, el); else nodes.current.delete(action.id); }} onFocus={() => setActive(action.id)} onClick={() => focus(rows.find(r => r.id === action.id))} onDoubleClick={() => choose(action)}><span className={css.actionIcon}>↳</span>{action.name}{favourites.includes(action.id) && <span aria-label="Favourite" className={css.star}>★</span>}</li>)}</ul>}
            </li>)}
          </ul>
        </div>
        <aside className={css.details}><span className={css.detailIcon}>⌘</span><p className={css.eyebrow}>{candidate?.category ?? 'MAKE IT YOURS'}</p><h3>{candidate?.name ?? 'One step, endless possibilities.'}</h3><p>{candidate?.description ?? 'Browse a category or search the library. Select an action to see its details and add it to your sequence.'}</p>{candidate && <button aria-pressed={favourites.includes(candidate.id)} onClick={favourite}>{favourites.includes(candidate.id) ? '★ Remove favourite' : '☆ Add favourite'}</button>}<span className={css.note}>B-3 SPIKE · FIXTURE DATA</span></aside>
      </div>
      <footer className={css.footer}><span><kbd>↑</kbd><kbd>↓</kbd> Navigate <kbd>↵</kbd> Add <kbd>Esc</kbd> Close</span><button disabled={!candidate} onClick={() => candidate && choose(candidate)}>Add action <span>↵</span></button></footer>
    </section>}
  </main>;
}
