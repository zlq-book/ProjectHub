// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import type { Project } from '../types';
import ProjectCard from './ProjectCard';
const project: Project = { id: 'a', name: '项目甲', path: 'D:\\Alpha', description: '', category: 'backend', tags: [], techStack: [], favorite: false, notes: '', defaultIde: 'system', createdAt: '2026-10-14', updatedAt: '2026-10-14', lastOpenedAt: null, available: true };
afterEach(() => { cleanup(); vi.useRealTimers(); });
it('鼠标先选中再展开，双击取消展开，保持卡片布局稳定', () => {
  vi.useFakeTimers(); const onSelect = vi.fn(), onOpen = vi.fn();
  render(<ProjectCard project={project} selected onSelect={onSelect} onOpen={onOpen} onFavorite={vi.fn()} onEdit={vi.fn()} />);
  const card = screen.getByRole('button', { name: '查看 项目甲' });
  fireEvent.click(card, { detail: 1 }); expect(onSelect).toHaveBeenLastCalledWith(false);
  fireEvent.doubleClick(card); act(() => vi.advanceTimersByTime(600));
  expect(onOpen).toHaveBeenCalledOnce(); expect(onSelect).toHaveBeenCalledTimes(1);
  fireEvent.click(card, { detail: 1 }); act(() => vi.advanceTimersByTime(500));
  expect(onSelect).toHaveBeenLastCalledWith(true);
});
it('切换选中项目后取消旧卡片的延迟展开', () => {
  vi.useFakeTimers(); const onSelect = vi.fn();
  const props = { project, onSelect, onOpen: vi.fn(), onFavorite: vi.fn(), onEdit: vi.fn() };
  const { rerender } = render(<ProjectCard {...props} selected />);
  fireEvent.click(screen.getByRole('button', { name: '查看 项目甲' }), { detail: 1 });
  rerender(<ProjectCard {...props} selected={false} />);
  act(() => vi.advanceTimersByTime(600));
  expect(onSelect).toHaveBeenCalledTimes(1);
});
