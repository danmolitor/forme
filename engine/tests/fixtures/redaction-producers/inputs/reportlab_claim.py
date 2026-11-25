"""Write the ReportLab redaction fixtures.

Usage: python reportlab_claim.py <out-dir> <ttf-path>

Writes two PDFs with the same lines (see ../README.md):
  reportlab-helvetica.pdf  standard 14 font (Helvetica, WinAnsi, not embedded)
  reportlab-truetype.pdf   embedded TrueType subset (IBM Plex Serif)

invariant=1 fixes the creation date and document ID so a rerun with the
same ReportLab version writes the same bytes.
"""

import os
import sys

from reportlab.lib.pagesizes import letter
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.pdfgen import canvas

LINES = [
    "Policy 7781",
    "Claimant",
    "Jane Doe",
    "SSN",
    "123-45-6789",
    "Email",
    "jane.doe@example.com",
    "Status Approved",
]


def write(path, font):
    c = canvas.Canvas(path, pagesize=letter, invariant=1)
    c.setTitle("Claim Summary")
    _, height = letter
    y = height - 72
    c.setFont(font, 22)
    c.drawString(72, y, "Claim Summary")
    y -= 36
    c.setFont(font, 14)
    for line in LINES:
        c.drawString(72, y, line)
        y -= 22
    c.showPage()
    c.save()


def main():
    out_dir, ttf = sys.argv[1], sys.argv[2]
    write(os.path.join(out_dir, "reportlab-helvetica.pdf"), "Helvetica")
    pdfmetrics.registerFont(TTFont("IBMPlexSerif", ttf))
    write(os.path.join(out_dir, "reportlab-truetype.pdf"), "IBMPlexSerif")


if __name__ == "__main__":
    main()
