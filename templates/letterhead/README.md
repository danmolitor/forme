# letterhead

A two-page business letter demonstrating the system's letterhead: full masthead and accent head rule on page one, a running header on the continuation, and a schedule table whose programme value totals the four releases. Change the recipient block, subject and prose first; the release table and its 51,866.30 total live in data.json. The running header ships without its hairline (margin-box borders do not render), and the bottom-left counter string is the letter reference on both pages (per-page strings need per-page margin boxes, which the engine scopes per document).

Engine features exercised: @page :first, running-header margin boxes, page counters, break-before: page, flex masthead, grouped table subtotal row.
