"""Tests for template serialization — no WASM needed."""
import json

from formepdf.templates import (
    Document, Page, View, Text, Image, Table, Row, Cell,
    Svg, QrCode, Barcode, PageBreak, Fixed, Watermark,
    _expand_edges, _map_style, _parse_color, _map_dimension,
)


class TestEdgeExpansion:
    def test_number(self):
        assert _expand_edges(40) == {"top": 40, "right": 40, "bottom": 40, "left": 40}

    def test_list_two(self):
        assert _expand_edges([10, 20]) == {"top": 10, "right": 20, "bottom": 10, "left": 20}

    def test_list_four(self):
        assert _expand_edges([1, 2, 3, 4]) == {"top": 1, "right": 2, "bottom": 3, "left": 4}

    def test_list_three(self):
        assert _expand_edges([10, 20, 30]) == {"top": 10, "right": 20, "bottom": 30, "left": 20}

    def test_string_shorthand(self):
        assert _expand_edges("8 16") == {"top": 8, "right": 16, "bottom": 8, "left": 16}

    def test_string_with_px(self):
        assert _expand_edges("8px 16px 24px 32px") == {"top": 8, "right": 16, "bottom": 24, "left": 32}


class TestStyleMapping:
    def test_font_weight_bold(self):
        result = _map_style({"fontWeight": "bold"})
        assert result["fontWeight"] == 700

    def test_font_weight_normal(self):
        result = _map_style({"fontWeight": "normal"})
        assert result["fontWeight"] == 400

    def test_font_weight_number(self):
        result = _map_style({"fontWeight": 600})
        assert result["fontWeight"] == 600

    def test_flex_direction(self):
        result = _map_style({"flexDirection": "column"})
        assert result["flexDirection"] == "Column"

    def test_flex_direction_row_reverse(self):
        result = _map_style({"flexDirection": "row-reverse"})
        assert result["flexDirection"] == "RowReverse"

    def test_justify_content(self):
        result = _map_style({"justifyContent": "space-between"})
        assert result["justifyContent"] == "SpaceBetween"

    def test_align_items(self):
        result = _map_style({"alignItems": "center"})
        assert result["alignItems"] == "Center"

    def test_dimension_number(self):
        assert _map_dimension(200) == {"Pt": 200}

    def test_dimension_percent(self):
        assert _map_dimension("50%") == {"Percent": 50}

    def test_dimension_auto(self):
        assert _map_dimension("auto") == "Auto"

    def test_flex_shorthand(self):
        result = _map_style({"flex": 1})
        assert result["flexGrow"] == 1
        assert result["flexShrink"] == 1
        assert result["flexBasis"] == {"Pt": 0}

    def test_color_hex(self):
        c = _parse_color("#ff0000")
        assert abs(c["r"] - 1.0) < 0.01
        assert abs(c["g"] - 0.0) < 0.01
        assert abs(c["b"] - 0.0) < 0.01
        assert c["a"] == 1.0

    def test_color_hex_short(self):
        c = _parse_color("#f00")
        assert abs(c["r"] - 1.0) < 0.01
        assert abs(c["g"] - 0.0) < 0.01

    def test_color_rgba(self):
        c = _parse_color("rgba(255, 128, 0, 0.5)")
        assert abs(c["r"] - 1.0) < 0.01
        assert abs(c["g"] - 128 / 255) < 0.01
        assert abs(c["b"] - 0.0) < 0.01
        assert abs(c["a"] - 0.5) < 0.01

    def test_color_rgb(self):
        c = _parse_color("rgb(0, 255, 0)")
        assert abs(c["g"] - 1.0) < 0.01

    def test_padding_edges(self):
        result = _map_style({"padding": 40})
        assert result["padding"] == {"top": 40, "right": 40, "bottom": 40, "left": 40}

    def test_padding_override(self):
        result = _map_style({"padding": 10, "paddingTop": 20})
        assert result["padding"]["top"] == 20
        assert result["padding"]["right"] == 10

    def test_border_shorthand(self):
        result = _map_style({"border": "1px solid #000"})
        assert result["borderWidth"]["top"] == 1
        assert result["borderColor"]["top"]["r"] == 0.0

    def test_text_align(self):
        result = _map_style({"textAlign": "center"})
        assert result["textAlign"] == "Center"

    def test_text_decoration(self):
        result = _map_style({"textDecoration": "line-through"})
        assert result["textDecoration"] == "LineThrough"

    def test_display_grid(self):
        result = _map_style({"display": "grid"})
        assert result["display"] == "Grid"

    def test_position_absolute(self):
        result = _map_style({"position": "absolute"})
        assert result["position"] == "Absolute"

    def test_overflow_hidden(self):
        result = _map_style({"overflow": "hidden"})
        assert result["overflow"] == "Hidden"

    def test_empty_style(self):
        assert _map_style(None) == {}
        assert _map_style({}) == {}


class TestDocumentSerialization:
    def test_basic_document(self):
        doc = Document(
            Page(Text("Hello")),
            title="Test",
            author="Author",
        )
        d = doc.to_dict()
        assert d["metadata"]["title"] == "Test"
        assert d["metadata"]["author"] == "Author"
        assert len(d["children"]) == 1
        page = d["children"][0]
        assert page["kind"]["type"] == "Page"

    def test_document_json(self):
        doc = Document(Page(Text("Hello")))
        j = doc.to_json()
        parsed = json.loads(j)
        assert "children" in parsed

    def test_document_with_lang(self):
        doc = Document(Page(Text("Hello")), lang="en-US")
        d = doc.to_dict()
        assert d["metadata"]["lang"] == "en-US"

    def test_document_with_style(self):
        doc = Document(
            Page(Text("Hello")),
            style={"fontSize": 14, "fontFamily": "Inter"},
        )
        d = doc.to_dict()
        assert d["default_style"]["fontSize"] == 14
        assert d["default_style"]["fontFamily"] == "Inter"

    def test_document_tagged(self):
        doc = Document(Page(Text("Hello")), tagged=True)
        d = doc.to_dict()
        assert d["tagged"] is True


class TestPageSerialization:
    def test_default_page(self):
        page = Page(Text("Hello"))
        d = page.to_dict()
        assert d["kind"]["config"]["size"] == "A4"
        assert d["kind"]["config"]["margin"] == {"top": 54, "right": 54, "bottom": 54, "left": 54}

    def test_custom_size(self):
        page = Page(Text("Hello"), size="Letter")
        d = page.to_dict()
        assert d["kind"]["config"]["size"] == "Letter"

    def test_custom_size_dict(self):
        page = Page(Text("Hello"), size={"width": 400, "height": 600})
        d = page.to_dict()
        assert d["kind"]["config"]["size"] == {"Custom": {"width": 400, "height": 600}}

    def test_custom_margin(self):
        page = Page(Text("Hello"), margin=36)
        d = page.to_dict()
        assert d["kind"]["config"]["margin"] == {"top": 36, "right": 36, "bottom": 36, "left": 36}


class TestViewSerialization:
    def test_view_with_children(self):
        v = View(
            Text("Child 1"),
            Text("Child 2"),
            style={"flexDirection": "column"},
        )
        d = v.to_dict()
        assert d["kind"]["type"] == "View"
        assert d["style"]["flexDirection"] == "Column"
        assert len(d["children"]) == 2

    def test_view_wrap(self):
        v = View(style={"padding": 10}, wrap=True)
        d = v.to_dict()
        assert d["style"]["wrap"] is True

    def test_view_bookmark(self):
        v = View(bookmark="section1")
        d = v.to_dict()
        assert d["bookmark"] == "section1"


class TestTextSerialization:
    def test_simple_text(self):
        t = Text("Hello world")
        d = t.to_dict()
        assert d["kind"]["type"] == "Text"
        assert d["kind"]["content"] == "Hello world"
        assert "runs" not in d["kind"]

    def test_text_with_runs(self):
        t = Text("", children=[
            "Normal text ",
            Text("bold text", style={"fontWeight": "bold"}),
        ])
        d = t.to_dict()
        assert "runs" in d["kind"]
        assert len(d["kind"]["runs"]) == 2
        assert d["kind"]["runs"][0] == {"content": "Normal text "}
        assert d["kind"]["runs"][1]["content"] == "bold text"
        assert d["kind"]["runs"][1]["style"]["fontWeight"] == 700

    def test_text_with_href(self):
        t = Text("Click me", href="https://example.com")
        d = t.to_dict()
        assert d["href"] == "https://example.com"


class TestImageSerialization:
    def test_image(self):
        img = Image("data:image/png;base64,abc", width=100, height=50)
        d = img.to_dict()
        assert d["kind"]["type"] == "Image"
        assert d["kind"]["src"] == "data:image/png;base64,abc"
        assert d["kind"]["width"] == 100
        assert d["kind"]["height"] == 50

    def test_image_with_alt(self):
        img = Image("file.png", alt="A photo")
        d = img.to_dict()
        assert d["alt"] == "A photo"


class TestTableSerialization:
    def test_table(self):
        tbl = Table(
            Row(Cell(Text("A")), Cell(Text("B")), header=True),
            Row(Cell(Text("1")), Cell(Text("2"))),
            columns=[{"width": 100}, {"width": "1fr"}],
        )
        d = tbl.to_dict()
        assert d["kind"]["type"] == "Table"
        assert len(d["kind"]["columns"]) == 2
        assert d["kind"]["columns"][0]["width"] == {"Fixed": 100}
        assert d["kind"]["columns"][1]["width"] == {"Fr": 1.0}
        assert len(d["children"]) == 2
        assert d["children"][0]["kind"]["is_header"] is True


class TestMiscComponents:
    def test_page_break(self):
        pb = PageBreak()
        d = pb.to_dict()
        assert d["kind"]["type"] == "PageBreak"

    def test_qr_code(self):
        qr = QrCode("https://example.com", size=100)
        d = qr.to_dict()
        assert d["kind"]["type"] == "QrCode"
        assert d["kind"]["data"] == "https://example.com"
        assert d["kind"]["size"] == 100

    def test_qr_code_with_color(self):
        qr = QrCode("test", size=80, color="#ff0000")
        d = qr.to_dict()
        assert d["kind"]["color"]["r"] == 1.0
        assert d["kind"]["color"]["g"] == 0.0

    def test_qr_code_minimal(self):
        qr = QrCode("data")
        d = qr.to_dict()
        assert d["kind"]["type"] == "QrCode"
        assert d["kind"]["data"] == "data"
        assert "size" not in d["kind"]
        assert "color" not in d["kind"]

    def test_qr_code_with_style(self):
        qr = QrCode("url", size=120, style={"margin": 8})
        d = qr.to_dict()
        assert d["style"]["margin"] == {"top": 8, "right": 8, "bottom": 8, "left": 8}

    def test_svg(self):
        s = Svg(200, 100, '<rect width="200" height="100" fill="blue"/>')
        d = s.to_dict()
        assert d["kind"]["type"] == "Svg"
        assert d["kind"]["width"] == 200

    def test_fixed(self):
        f = Fixed(Text("Header"), position="top")
        d = f.to_dict()
        assert d["kind"]["type"] == "Fixed"
        assert d["kind"]["position"] == "Top"

    def test_watermark(self):
        w = Watermark("DRAFT", font_size=60, color="rgba(0,0,0,0.1)", angle=-45)
        d = w.to_dict()
        assert d["kind"]["type"] == "Watermark"
        assert d["kind"]["text"] == "DRAFT"
        assert d["kind"]["font_size"] == 60
        assert d["kind"]["angle"] == -45


class TestBarcode:
    def test_barcode_default_format(self):
        bc = Barcode("ABC-123")
        d = bc.to_dict()
        assert d["kind"]["type"] == "Barcode"
        assert d["kind"]["data"] == "ABC-123"
        assert d["kind"]["format"] == "Code128"
        assert d["kind"]["height"] == 60.0

    def test_barcode_custom_format(self):
        bc = Barcode("HELLO", format="Code39", width=200, height=40)
        d = bc.to_dict()
        assert d["kind"]["format"] == "Code39"
        assert d["kind"]["width"] == 200
        assert d["kind"]["height"] == 40

    def test_barcode_with_color(self):
        bc = Barcode("12345", color="#003366")
        d = bc.to_dict()
        assert "color" in d["style"]

    def test_barcode_no_width_omitted(self):
        bc = Barcode("test")
        d = bc.to_dict()
        assert "width" not in d["kind"]


class TestEmbedData:
    def test_embed_data_in_document(self):
        doc = Document(Page(Text("Hello")))
        d = doc.to_dict()
        assert "embedded_data" not in d

        # render() would inject embed_data, but we test to_dict level
        # The embed_data injection happens in Document.render()


def test_font_feature_style_props_map_to_font_features():
    from formepdf.templates import _map_style

    assert _map_style({"fontFeatureSettings": '"tnum" 1, "zero", "liga" off'})["fontFeatures"] == [
        {"tag": "tnum", "value": 1},
        {"tag": "zero", "value": 1},
        {"tag": "liga", "value": 0},
    ]
    # As in CSS, font-feature-settings wins over font-variant-numeric.
    assert _map_style({"fontVariantNumeric": "tabular-nums", "fontFeatureSettings": '"tnum" 0'})[
        "fontFeatures"
    ] == [{"tag": "tnum", "value": 0}]
    assert _map_style({"fontVariantNumeric": "normal"})["fontFeatures"] == []
    assert "fontFeatures" not in _map_style({"fontFeatureSettings": "nonsense"})
