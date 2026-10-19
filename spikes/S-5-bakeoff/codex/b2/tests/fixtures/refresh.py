"""Refresh data-only examples from official Twitch docs; no third-party code is run.
Run from any directory with Python 3. Only writes JSON files beside this script.
"""
import json
import re
import urllib.request
from html.parser import HTMLParser
from pathlib import Path

BASE = 'https://dev.twitch.tv/docs/eventsub/'

class Examples(HTMLParser):
    def __init__(self):
        super().__init__()
        self.heading = ''
        self.in_heading = False
        self.in_pre = False
        self.buffer = ''
        self.examples = {}

    def handle_starttag(self, tag, attrs):
        if tag in ('h2', 'h3'):
            self.in_heading = True
            self.heading = ''
        if tag == 'pre':
            self.in_pre = True
            self.buffer = ''

    def handle_data(self, data):
        if self.in_heading:
            self.heading += data
        if self.in_pre:
            self.buffer += data

    def handle_endtag(self, tag):
        if tag in ('h2', 'h3'):
            self.in_heading = False
        if tag == 'pre':
            self.in_pre = False
            self.examples[self.heading] = self.buffer


def fetch(page):
    parser = Examples()
    with urllib.request.urlopen(BASE + page + '/') as response:
        parser.feed(response.read().decode())
    return parser.examples


def parse(text):
    # Docs include explanatory line comments outside strings in some examples.
    return json.loads(re.sub(r'(?m)[ \t]+//[^\n]*', '', text))


def write(name, value):
    Path(__file__).with_name(name + '.json').write_text(json.dumps(value, indent=2) + '\n')


if __name__ == '__main__':
    ws = fetch('handling-websocket-events')
    for name, title in [('welcome', 'Welcome message'), ('keepalive', 'Keepalive message'),
                        ('reconnect', 'Reconnect message'), ('revocation', 'Revocation message')]:
        write(name, parse(ws[title]))
    events = fetch('eventsub-subscription-types')
    titles = {
        'chat': 'Regular Chat Message Example',
        'shared_chat': 'Shared Chat Message Example',
        'follow': 'Channel Follow Webhook Notification Example',
        'subscribe': 'Channel Subscribe Webhook Notification Example',
        'gift': 'Channel Subscription Gift Webhook Notification Example',
        'subscription_message': 'Channel Subscription Message Webhook Notification Example',
        'cheer': 'Channel Cheer Notification Example',
        'raid': 'Channel Raid Notification Example',
        'redemption': 'Channel Points Custom Reward Redemption Add Notification Example',
        'hype_train': 'Channel Hype Train Begin Notification Example',
    }
    for name, title in titles.items():
        payload = parse(events[title])
        subscription = payload['subscription']
        # Event bodies are unchanged. Adapt webhook transport to WebSocket.
        subscription['transport'] = {'method': 'websocket', 'session_id': 'fixture-session'}
        metadata = {
            'message_id': 'fixture-' + name,
            'message_type': 'notification',
            'message_timestamp': '2023-07-19T14:56:51.634234626Z',
            'subscription_type': subscription['type'],
            'subscription_version': subscription['version'],
        }
        write(name, {'metadata': metadata, 'payload': payload})
