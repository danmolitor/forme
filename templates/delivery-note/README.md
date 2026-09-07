# delivery-note

The consignee's delivery note: quantities and weights (which match the packing slip and label), a receipt-of-goods statement, condition checkboxes, exception write-in lines, and a four-slot signature row where no two captions repeat. Change the consignee, line rows and totals in `data.json` first; quantity 72 is 73 shipped minus the DOC-QA envelope, and the 418 lb total must match the shipment.

Engine features: checkbox lines as flex rows, write-in rules, four-slot signature row with narrow slots, single-page fill.

Render: `npx @formepdf/html templates/delivery-note/index.html -o delivery-note.pdf`
