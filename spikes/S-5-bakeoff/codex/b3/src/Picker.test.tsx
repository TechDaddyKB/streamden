import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { Picker } from './Picker';
import { actions } from './model';
const tree = () => screen.getByRole('tree', { name: 'Sub-actions' });
const item = (name: string) => within(tree()).getByRole('treeitem', { name });
async function start() { const user = userEvent.setup(); render(<Picker />); return user; }
describe('sub-action picker', () => {
  it('loads 200 unique generated actions and accessible nested categories', async () => {
    await start();
    expect(actions).toHaveLength(200); expect(new Set(actions.map(a => a.id)).size).toBe(200);
    expect(item('Core')).toHaveAttribute('aria-expanded', 'true');
    expect(item('Twitch')).toHaveAttribute('aria-expanded', 'false');
    expect(within(item('Core')).getByRole('group')).toBeVisible();
  });
  it('fuzzy-searches abbreviations, expands ancestors and reports no matches', async () => {
    const user = await start();
    await user.type(screen.getByLabelText('Search actions'), 'stscn');
    expect(item('Set scene')).toBeVisible(); expect(item('OBS')).toHaveAttribute('aria-expanded', 'true');
    await user.clear(screen.getByLabelText('Search actions'));
    await user.type(screen.getByLabelText('Search actions'), 'zzzzzz');
    expect(screen.getByText(/No actions found/)).toBeVisible();
    expect(within(tree()).queryAllByRole('treeitem')).toHaveLength(0);
  });
  it('moves through visible rows, expands and collapses, and jumps to edges', async () => {
    const user = await start();
    await user.keyboard('{ArrowDown}{ArrowLeft}');
    expect(item('Core')).toHaveFocus(); expect(item('Core')).toHaveAttribute('aria-expanded', 'false');
    await user.keyboard('{ArrowRight}{ArrowRight}'); expect(item('Set variable')).toHaveFocus();
    await user.keyboard('{ArrowDown}'); expect(item('Get variable')).toHaveFocus();
    await user.keyboard('{ArrowUp}{ArrowLeft}'); expect(item('Core')).toHaveFocus();
    await user.keyboard('{End}'); expect(item('Integrations')).toHaveFocus();
    await user.keyboard('{ArrowDown}'); expect(item('Integrations')).toHaveFocus();
    await user.keyboard('{Home}{ArrowUp}'); expect(item('Core')).toHaveFocus();
  });
  it('supports type-ahead and enters a leaf to select it and restore focus', async () => {
    const user = await start();
    await user.keyboard('{ArrowDown}tw'); expect(item('Twitch')).toHaveFocus();
    await user.keyboard('{ArrowRight}{ArrowRight}{Enter}');
    expect(screen.getByRole('status')).toHaveTextContent('Added: Set chat message (Twitch)');
    expect(screen.queryByRole('tree')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '＋ Add sub-action' })).toHaveFocus();
    await user.click(screen.getByRole('button', { name: '＋ Add sub-action' }));
    await user.click(screen.getByRole('button', { name: /Recent/ }));
    expect(item('Set chat message')).toBeVisible();
  });
  it('favourites persist, filter and remove without corrupting focus or selection', async () => {
    const user = await start();
    await user.click(item('Set variable'));
    await user.click(screen.getByRole('button', { name: '☆ Add favourite' }));
    await user.click(screen.getByRole('button', { name: /Favourites/ }));
    expect(within(tree()).getAllByRole('treeitem')).toHaveLength(2);
    await user.click(item('Set variable'));
    expect(screen.getByRole('button', { name: '★ Remove favourite' })).toHaveAttribute('aria-pressed', 'true');
    expect(JSON.parse(localStorage.getItem('b3-favourites')!)).toEqual(['core-0-0']);
    await user.click(screen.getByRole('button', { name: '★ Remove favourite' }));
    expect(screen.getByText(/No actions found/)).toBeVisible();
  });
  it('closes with Escape and supports mouse selection using Add action', async () => {
    const user = await start();
    await user.keyboard('{Escape}'); expect(screen.queryByRole('tree')).toBeNull();
    await user.click(screen.getByRole('button', { name: '＋ Add sub-action' }));
    await user.click(item('Set variable')); await user.click(screen.getByRole('button', { name: /Add action/ }));
    expect(screen.getByRole('status')).toHaveTextContent('Added: Set variable (Core)');
  });
  it('loads only valid known IDs and tolerates malformed storage', async () => {
    localStorage.setItem('b3-favourites', '{bad json');
    localStorage.setItem('b3-recent', '[null,3,"missing","twitch-0-0","twitch-0-0"]');
    const user = await start();
    await user.click(screen.getByRole('button', { name: /Recent/ }));
    await user.click(screen.getByText('Twitch', { selector: 'strong' }));
    expect(item('Set chat message')).toBeVisible();
    expect(screen.getByRole('button', { name: /Recent/ })).toHaveTextContent('Recent · 1');
  });
  it('keeps one tabbable treeitem when filtering hides the active leaf', async () => {
    const user = await start(); await user.click(item('Set variable'));
    await user.click(screen.getByLabelText('Search actions'));
    await user.type(screen.getByLabelText('Search actions'), 'obs');
    expect(within(tree()).getAllByRole('treeitem').filter(el => el.tabIndex === 0)).toHaveLength(1);
    await user.keyboard('{ArrowDown}{ArrowRight}'); expect(item('Set scene')).toHaveFocus();
  });
  it('remains usable when browser storage is blocked', async () => {
    const read = vi.spyOn(localStorage, 'getItem').mockImplementation(() => { throw new Error('blocked'); });
    const write = vi.spyOn(localStorage, 'setItem').mockImplementation(() => { throw new Error('blocked'); });
    try {
      const user = await start(); await user.click(item('Set variable'));
      await user.click(screen.getByRole('button', { name: '☆ Add favourite' }));
      expect(screen.getByRole('button', { name: '★ Remove favourite' })).toBeVisible();
      await user.click(screen.getByRole('button', { name: /Add action/ }));
      expect(screen.getByRole('status')).toHaveTextContent('Added: Set variable (Core)');
    } finally { read.mockRestore(); write.mockRestore(); }
  });
});
