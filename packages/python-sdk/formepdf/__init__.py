"""Forme Python SDK — client for the Forme hosted PDF API and local rendering."""

from .client import Forme, FormeError
from .wasm import render_html
from .templates import (
    Document,
    Page,
    View,
    Text,
    Image,
    Table,
    Row,
    Cell,
    Svg,
    QrCode,
    Barcode,
    PageBreak,
    Fixed,
    Watermark,
    BarChart,
    LineChart,
    PieChart,
    AreaChart,
    DotPlot,
    TextField,
    Checkbox,
    Dropdown,
    RadioButton,
)

__all__ = [
    # API client
    "Forme",
    "FormeError",
    # Local HTML → PDF (WASM)
    "render_html",
    # Template components
    "Document",
    "Page",
    "View",
    "Text",
    "Image",
    "Table",
    "Row",
    "Cell",
    "Svg",
    "QrCode",
    "Barcode",
    "PageBreak",
    "Fixed",
    "Watermark",
    # Charts
    "BarChart",
    "LineChart",
    "PieChart",
    "AreaChart",
    "DotPlot",
    # Form fields
    "TextField",
    "Checkbox",
    "Dropdown",
    "RadioButton",
]
