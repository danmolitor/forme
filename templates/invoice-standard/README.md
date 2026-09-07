# invoice-standard

A one-page standard invoice in the Northmoor document system: masthead, four-column meta grid, itemised table, totals stack and an accent amount-due block. Change the parties, line items and totals in index.html first (data.json mirrors every field). The footer is pushed to the page foot by a fixed-height flex column sized to Letter's content box; switch to A4 by changing @page size and the .pagefill height only.

Engine features exercised: @page margin boxes with counter(page)/counter(pages), border-collapse tables, flexbox with flex-grow, absolute positioning for the full-bleed head rule.
