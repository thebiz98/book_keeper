import { describe, it, expect } from 'vitest';
import { render } from '../testUtils';
import { DataTable } from '@/components/domain/DataTable';
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from '@/components/ui/Table';

// Sticky headers (FR-10 rev., DD-43). happy-dom has no layout, so these are structural checks.

function sample(props: React.ComponentProps<typeof Table> = {}) {
  return (
    <Table {...props}>
      <TableHeader>
        <TableRow>
          <TableHead>Name</TableHead>
          <TableHead align="right">Qty</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow>
          <TableCell>A</TableCell>
          <TableCell align="right">1</TableCell>
        </TableRow>
      </TableBody>
    </Table>
  );
}

describe('Table - sticky header and scroll modes', () => {
  it('TC-TBL-01: bounded (default) scrolls itself and every th is sticky, opaque', () => {
    const { container } = render(sample());
    const box = container.querySelector('[data-table-scroll]') as HTMLElement;
    expect(box.getAttribute('data-table-scroll')).toBe('bounded');
    expect(box.className).toContain('overflow-auto');
    expect(box.className).toContain('max-h-[60vh]');
    const ths = Array.from(container.querySelectorAll('th'));
    expect(ths.length).toBe(2);
    for (const th of ths) {
      expect(th.className).toContain('sticky');
      expect(th.className).toContain('top-0');
      expect(th.className).toContain('z-10');
      expect(th.className).toContain('bg-slate-50');
      expect(th.className).toContain('dark:bg-slate-800');
      expect(th.className).not.toMatch(/\bbg-[a-z]+-\d+\/\d+/);
      expect(th.className).not.toMatch(/dark:bg-[a-z]+-\d+\/\d+/);
    }
  });

  it('TC-TBL-02: fill mode fills a flex parent without a max height', () => {
    const { container } = render(sample({ scroll: 'fill' }));
    const box = container.querySelector('[data-table-scroll]') as HTMLElement;
    expect(box.className).toContain('flex-1');
    expect(box.className).toContain('min-h-0');
    expect(box.className).not.toContain('max-h-');
  });

  it('TC-TBL-03: page mode has no scroll box of its own but the header is still sticky', () => {
    const { container } = render(sample({ scroll: 'page' }));
    const box = container.querySelector('[data-table-scroll]') as HTMLElement;
    expect(box.className).toContain('overflow-visible');
    expect(container.querySelector('th')!.className).toContain('sticky');
  });

  it('TC-TBL-04: none mode (nested tables) is not sticky', () => {
    const { container } = render(sample({ scroll: 'none' }));
    const box = container.querySelector('[data-table-scroll]') as HTMLElement;
    expect(box.className).toContain('overflow-x-auto');
    expect(container.querySelector('th')!.className).not.toContain('sticky');
  });

  it('TC-TBL-05: containerClassName overrides the default max height', () => {
    const { container } = render(sample({ containerClassName: 'max-h-64' }));
    const box = container.querySelector('[data-table-scroll]') as HTMLElement;
    expect(box.className).toContain('max-h-64');
    expect(box.className).not.toContain('max-h-[60vh]');
  });
});

describe('DataTable - single scroll container (sticky pitfall)', () => {
  it('TC-TBL-06: no overflow element sits between th and the scroll box, and there is exactly one box', () => {
    const { container } = render(
      <DataTable
        columns={[{ key: 'n', header: 'Name', render: (r: { id: number }) => `Row ${r.id}` }]}
        items={[{ id: 1 }, { id: 2 }]}
        rowKey={(r) => r.id}
        scroll="fill"
      />
    );
    expect(container.querySelectorAll('[data-table-scroll]').length).toBe(1);
    const box = container.querySelector('[data-table-scroll]') as HTMLElement;
    expect(box.getAttribute('data-table-scroll')).toBe('fill');
    let el = container.querySelector('th')!.parentElement;
    while (el && el !== box) {
      expect(el.className).not.toMatch(/overflow-/);
      el = el.parentElement;
    }
    expect(el).toBe(box);
  });
});

describe('DataTable footer row (J1)', () => {
  const cols = [
    { key: 'a', header: 'A', render: (r: { id: number }) => r.id },
    { key: 'b', header: 'B', align: 'right' as const, render: (r: { id: number }) => r.id },
  ];

  it('renders one sticky-bottom opaque cell per column, keyed by column key', () => {
    const { container } = render(
      <DataTable columns={cols} items={[{ id: 1 }]} rowKey={(r) => r.id} scroll="fill" footer={{ a: 'Total', b: '9' }} footerTestId="ft" />
    );
    const cells = Array.from(container.querySelectorAll('[data-testid="ft"] td')) as HTMLElement[];
    expect(cells.map((c) => c.textContent)).toEqual(['Total', '9']);
    for (const c of cells) {
      expect(c.className).toMatch(/sticky bottom-0/);
      expect(c.className).toMatch(/bg-slate-50/);
    }
    expect(cells[1].className).toMatch(/text-right/);
  });

  it('has no footer without the prop, and none while empty', () => {
    const a = render(<DataTable columns={cols} items={[{ id: 1 }]} rowKey={(r) => r.id} />);
    expect(a.container.querySelector('tfoot')).toBeNull();
    const b = render(<DataTable columns={cols} items={[]} rowKey={(r: { id: number }) => r.id} footer={{ a: 'x' }} />);
    expect(b.container.querySelector('tfoot')).toBeNull();
  });
});
