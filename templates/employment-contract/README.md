# employment-contract

A three-page employment contract with thirteen numbered clauses grouped 1–3 / 4–8 / 9–13 by deliberate page breaks, an execution block that never separates from its closing text, and an Initials line in every page's footer. Change the parties, particulars and salary first — the reference, start date and salary must stay consistent with the offer-letter and policy-acknowledgement templates (data.json carries all three references). The running header ships without its hairline (margin-box borders do not render), and the bottom-left counter string is the contract reference on every page.

Engine features exercised: @page :first, running-header margin boxes, page counters, break-before: page groups, break-inside: avoid on the execution block, flex clause layout with fixed number column.
