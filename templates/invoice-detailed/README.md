# invoice-detailed

A three-page progress-billing invoice with grouped line-item sections, section subtotals, a carried-forward/brought-forward pair at the page 2/3 boundary, and a retainage-and-tax totals stack. Change the sections and their sums first — the summary table, carried-forward figure and totals must keep reconciling (data.json carries the chain). Deviations from the prototype, by engine policy: the running header is three margin boxes without the hairline beneath (margin-box borders do not render), and both continuation pages share one section label, 'Detail, sections A–E' (a per-page label via named pages currently leaves a trailing blank page).

Engine features exercised: @page :first, three running-header margin boxes, page counters, break-before: page, repeated <thead>, grouped table sections with colspan, accent keyline subtotal rows.
