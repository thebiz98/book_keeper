import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render } from '../testUtils';
import { LiveSearchTypeahead } from '@/components/common/LiveSearchTypeahead';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import { typeInto, press, flush } from './saleFixtures';

interface Row {
  id: number;
  name: string;
}

const input = () => document.querySelector<HTMLInputElement>('input')!;
const options = () => Array.from(document.querySelectorAll('[role="option"]')).map((o) => o.textContent ?? '');

function setup(rows: Row[], extra: Partial<React.ComponentProps<typeof LiveSearchTypeahead<Row>>> = {}) {
  const onCreate = vi.fn();
  const onSelect = vi.fn();
  render(
    <LiveSearchTypeahead<Row>
      search={async () => rows}
      getKey={(r) => r.id}
      getLabel={(r) => r.name}
      renderItem={(r) => <span>{r.name}</span>}
      onSelect={onSelect}
      onCreate={onCreate}
      ariaLabel="Search types"
      {...extra}
    />
  );
  return { onCreate, onSelect };
}

describe('LiveSearchTypeahead', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    __resetHotkeys();
  });
  afterEach(() => vi.useRealTimers());

  it('shows the create row only when there is no case-insensitive exact match', async () => {
    setup([{ id: 1, name: 'Rent' }]);
    typeInto(input(), 'rent');
    await flush(60);
    expect(options()).toEqual(['Rent']);
    typeInto(input(), 'ren');
    await flush(60);
    expect(options()).toEqual(['Rent', '+ Create "ren"']);
  });

  it('Enter on an empty result calls onCreate with the trimmed query', async () => {
    const { onCreate, onSelect } = setup([]);
    typeInto(input(), '  Electricity ');
    await flush(60);
    expect(options()).toEqual(['+ Create "Electricity"']);
    press(input(), 'Enter');
    await flush(0);
    expect(onCreate).toHaveBeenCalledWith('Electricity');
    expect(onSelect).not.toHaveBeenCalled();
  });

  it('arrow keys reach the create row and Enter on it creates', async () => {
    const { onCreate, onSelect } = setup([{ id: 1, name: 'Rent' }]);
    typeInto(input(), 'ren');
    await flush(60);
    press(input(), 'ArrowDown');
    await flush(0);
    press(input(), 'Enter');
    await flush(0);
    expect(onCreate).toHaveBeenCalledWith('ren');
    expect(onSelect).not.toHaveBeenCalled();
  });

  it('Enter on the highlighted result selects it', async () => {
    const { onCreate, onSelect } = setup([{ id: 1, name: 'Rent' }, { id: 2, name: 'Rental' }]);
    typeInto(input(), 'ren');
    await flush(60);
    press(input(), 'Enter');
    await flush(0);
    expect(onSelect).toHaveBeenCalledWith({ id: 1, name: 'Rent' });
    expect(onCreate).not.toHaveBeenCalled();
  });

  it('Esc closes the list', async () => {
    setup([{ id: 1, name: 'Rent' }]);
    typeInto(input(), 'ren');
    await flush(60);
    expect(options().length).toBeGreaterThan(0);
    press(document, 'Escape');
    await flush(0);
    expect(options()).toEqual([]);
  });

  it('without onCreate an empty result shows the empty text', async () => {
    setup([], { onCreate: undefined, emptyText: (q) => `Nothing for "${q}"` });
    typeInto(input(), 'zzz');
    await flush(60);
    expect(document.body.textContent).toContain('Nothing for "zzz"');
  });
});
