"""Pythonic component DSL for building Forme PDF documents.

Components serialize to the JSON schema that the Rust engine expects.
WASM is only needed at render time (Document.render()), not for building
or serializing the document tree.
"""
from __future__ import annotations

import json
import re
from typing import Any, Dict, List, Optional, Sequence, Tuple, Union

# ─── Style mapping ───────────────────────────────────────────────────

_FLEX_DIRECTION_MAP = {
    "row": "Row",
    "column": "Column",
    "row-reverse": "RowReverse",
    "column-reverse": "ColumnReverse",
}

_JUSTIFY_CONTENT_MAP = {
    "flex-start": "FlexStart",
    "flex-end": "FlexEnd",
    "center": "Center",
    "space-between": "SpaceBetween",
    "space-around": "SpaceAround",
    "space-evenly": "SpaceEvenly",
}

_ALIGN_ITEMS_MAP = {
    "flex-start": "FlexStart",
    "flex-end": "FlexEnd",
    "center": "Center",
    "stretch": "Stretch",
    "baseline": "Baseline",
}

_FLEX_WRAP_MAP = {
    "nowrap": "NoWrap",
    "wrap": "Wrap",
}

_ALIGN_CONTENT_MAP = {
    "flex-start": "FlexStart",
    "flex-end": "FlexEnd",
    "center": "Center",
    "space-between": "SpaceBetween",
    "space-around": "SpaceAround",
    "space-evenly": "SpaceEvenly",
    "stretch": "Stretch",
}

_FONT_STYLE_MAP = {
    "normal": "Normal",
    "italic": "Italic",
    "oblique": "Oblique",
}

_TEXT_ALIGN_MAP = {
    "left": "Left",
    "right": "Right",
    "center": "Center",
    "justify": "Justify",
}

_TEXT_DECORATION_MAP = {
    "none": "None",
    "underline": "Underline",
    "line-through": "LineThrough",
}

_TEXT_TRANSFORM_MAP = {
    "none": "None",
    "uppercase": "Uppercase",
    "lowercase": "Lowercase",
    "capitalize": "Capitalize",
}

_HYPHENS_MAP = {
    "none": "none",
    "manual": "manual",
    "auto": "auto",
}

_TEXT_OVERFLOW_MAP = {
    "wrap": "Wrap",
    "ellipsis": "Ellipsis",
    "clip": "Clip",
}

_LINE_BREAKING_MAP = {
    "optimal": "optimal",
    "greedy": "greedy",
}

_OVERFLOW_MAP = {
    "visible": "Visible",
    "hidden": "Hidden",
}

_DISPLAY_MAP = {
    "flex": "Flex",
    "grid": "Grid",
}

_POSITION_MAP = {
    "relative": "Relative",
    "absolute": "Absolute",
}


def _parse_color(color: str) -> Dict[str, float]:
    """Parse a CSS color string to {r, g, b, a} with 0-1 values."""
    s = color.strip()

    # rgba(r, g, b, a)
    m = re.match(
        r"^rgba\(\s*(\d+(?:\.\d+)?)\s*,\s*(\d+(?:\.\d+)?)\s*,\s*(\d+(?:\.\d+)?)\s*,\s*(\d+(?:\.\d+)?)\s*\)$",
        s,
    )
    if m:
        return {
            "r": float(m.group(1)) / 255,
            "g": float(m.group(2)) / 255,
            "b": float(m.group(3)) / 255,
            "a": float(m.group(4)),
        }

    # rgb(r, g, b)
    m = re.match(
        r"^rgb\(\s*(\d+(?:\.\d+)?)\s*,\s*(\d+(?:\.\d+)?)\s*,\s*(\d+(?:\.\d+)?)\s*\)$",
        s,
    )
    if m:
        return {
            "r": float(m.group(1)) / 255,
            "g": float(m.group(2)) / 255,
            "b": float(m.group(3)) / 255,
            "a": 1.0,
        }

    # Hex: #RGB, #RRGGBB, #RRGGBBAA
    if s.startswith("#"):
        h = s[1:]
        if len(h) == 3:
            h = h[0] * 2 + h[1] * 2 + h[2] * 2
        if len(h) == 6:
            return {
                "r": int(h[0:2], 16) / 255,
                "g": int(h[2:4], 16) / 255,
                "b": int(h[4:6], 16) / 255,
                "a": 1.0,
            }
        if len(h) == 8:
            return {
                "r": int(h[0:2], 16) / 255,
                "g": int(h[2:4], 16) / 255,
                "b": int(h[4:6], 16) / 255,
                "a": int(h[6:8], 16) / 255,
            }

    # Fallback: black
    return {"r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0}


def _map_dimension(val: Union[int, float, str]) -> Any:
    """Map a dimension value to the Forme JSON format."""
    if isinstance(val, (int, float)):
        return {"Pt": val}
    if val == "auto":
        return "Auto"
    m = re.match(r"^([0-9.]+)%$", val)
    if m:
        return {"Percent": float(m.group(1))}
    try:
        return {"Pt": float(val)}
    except (ValueError, TypeError):
        return "Auto"


def _expand_edges(val: Union[int, float, str, list, tuple]) -> Dict[str, float]:
    """Expand a shorthand edge value to {top, right, bottom, left}."""
    if isinstance(val, (int, float)):
        return {"top": val, "right": val, "bottom": val, "left": val}
    if isinstance(val, (list, tuple)):
        if len(val) == 1:
            return {"top": val[0], "right": val[0], "bottom": val[0], "left": val[0]}
        if len(val) == 2:
            return {"top": val[0], "right": val[1], "bottom": val[0], "left": val[1]}
        if len(val) == 3:
            return {"top": val[0], "right": val[1], "bottom": val[2], "left": val[1]}
        if len(val) >= 4:
            return {"top": val[0], "right": val[1], "bottom": val[2], "left": val[3]}
    if isinstance(val, str):
        parts = val.replace("px", "").split()
        nums = [float(p) for p in parts]
        return _expand_edges(nums)
    return {"top": 0, "right": 0, "bottom": 0, "left": 0}


def _expand_margin_edges(val: Union[int, float, str, list, tuple]) -> Dict[str, Any]:
    """Expand a shorthand margin value, preserving 'auto' string values."""
    if val == "auto":
        return {"top": "auto", "right": "auto", "bottom": "auto", "left": "auto"}
    return _expand_edges(val)


def _expand_corners(val: Union[int, float]) -> Dict[str, float]:
    """Expand a border radius shorthand."""
    v = float(val)
    return {"top_left": v, "top_right": v, "bottom_right": v, "bottom_left": v}


def _parse_border_string(val: str) -> Tuple[Optional[float], Optional[Dict[str, float]]]:
    """Parse a CSS border shorthand like '1px solid #000'. Returns (width, color)."""
    _BORDER_STYLE_KEYWORDS = {
        "solid", "dashed", "dotted", "double", "groove", "ridge",
        "inset", "outset", "none", "hidden",
    }
    tokens = val.split()
    width = None
    color = None
    for token in tokens:
        if token.lower() in _BORDER_STYLE_KEYWORDS:
            continue
        if token.startswith("#") or token.startswith("rgb"):
            color = _parse_color(token)
            continue
        try:
            width = float(token.replace("px", ""))
        except ValueError:
            pass
    return width, color


def _map_style(style: Optional[Dict[str, Any]]) -> Dict[str, Any]:
    """Map a user-facing style dict to the Forme JSON schema."""
    if not style:
        return {}

    result: Dict[str, Any] = {}

    # Dimensions
    for key in ("width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight"):
        if key in style:
            result[key] = _map_dimension(style[key])

    # Edges: padding
    _pad_keys = ("padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft",
                 "paddingHorizontal", "paddingVertical")
    if any(k in style for k in _pad_keys):
        base = _expand_edges(style["padding"]) if "padding" in style else {"top": 0, "right": 0, "bottom": 0, "left": 0}
        vt = style.get("paddingVertical", base["top"])
        vb = style.get("paddingVertical", base["bottom"])
        hl = style.get("paddingHorizontal", base["left"])
        hr = style.get("paddingHorizontal", base["right"])
        result["padding"] = {
            "top": style.get("paddingTop", vt),
            "right": style.get("paddingRight", hr),
            "bottom": style.get("paddingBottom", vb),
            "left": style.get("paddingLeft", hl),
        }

    # Edges: margin
    _margin_keys = ("margin", "marginTop", "marginRight", "marginBottom", "marginLeft",
                    "marginHorizontal", "marginVertical")
    if any(k in style for k in _margin_keys):
        base = _expand_margin_edges(style["margin"]) if "margin" in style else {"top": 0, "right": 0, "bottom": 0, "left": 0}
        vt = style.get("marginVertical", base["top"])
        vb = style.get("marginVertical", base["bottom"])
        hl = style.get("marginHorizontal", base["left"])
        hr = style.get("marginHorizontal", base["right"])
        result["margin"] = {
            "top": style.get("marginTop", vt),
            "right": style.get("marginRight", hr),
            "bottom": style.get("marginBottom", vb),
            "left": style.get("marginLeft", hl),
        }

    # Flex shorthand
    if "flex" in style:
        if "flexGrow" not in style:
            result["flexGrow"] = style["flex"]
        if "flexShrink" not in style:
            result["flexShrink"] = 1
        if "flexBasis" not in style:
            result["flexBasis"] = {"Pt": 0}

    # Flex enums
    if "flexDirection" in style:
        result["flexDirection"] = _FLEX_DIRECTION_MAP.get(style["flexDirection"], style["flexDirection"])
    if "justifyContent" in style:
        result["justifyContent"] = _JUSTIFY_CONTENT_MAP.get(style["justifyContent"], style["justifyContent"])
    if "alignItems" in style:
        result["alignItems"] = _ALIGN_ITEMS_MAP.get(style["alignItems"], style["alignItems"])
    if "alignSelf" in style:
        result["alignSelf"] = _ALIGN_ITEMS_MAP.get(style["alignSelf"], style["alignSelf"])
    if "flexWrap" in style:
        result["flexWrap"] = _FLEX_WRAP_MAP.get(style["flexWrap"], style["flexWrap"])
    if "alignContent" in style:
        result["alignContent"] = _ALIGN_CONTENT_MAP.get(style["alignContent"], style["alignContent"])

    # Flex numeric pass-through
    for key in ("flexGrow", "flexShrink", "gap", "rowGap", "columnGap"):
        if key in style:
            result[key] = style[key]
    if "flexBasis" in style:
        result["flexBasis"] = _map_dimension(style["flexBasis"])

    # Display
    if "display" in style:
        result["display"] = _DISPLAY_MAP.get(style["display"], "Flex")

    # Grid
    if "gridTemplateColumns" in style:
        result["gridTemplateColumns"] = _parse_grid_template(style["gridTemplateColumns"])
    if "gridTemplateRows" in style:
        result["gridTemplateRows"] = _parse_grid_template(style["gridTemplateRows"])
    if "gridAutoRows" in style:
        result["gridAutoRows"] = _map_grid_track(style["gridAutoRows"])
    if "gridAutoColumns" in style:
        result["gridAutoColumns"] = _map_grid_track(style["gridAutoColumns"])
    # Grid placement
    _grid_placement_keys = ("gridColumnStart", "gridColumnEnd", "gridRowStart", "gridRowEnd",
                            "gridColumnSpan", "gridRowSpan")
    if any(k in style for k in _grid_placement_keys):
        placement: Dict[str, Any] = {}
        key_map = {
            "gridColumnStart": "columnStart",
            "gridColumnEnd": "columnEnd",
            "gridRowStart": "rowStart",
            "gridRowEnd": "rowEnd",
            "gridColumnSpan": "columnSpan",
            "gridRowSpan": "rowSpan",
        }
        for style_key, json_key in key_map.items():
            if style_key in style:
                placement[json_key] = style[style_key]
        result["gridPlacement"] = placement

    # Typography
    if "fontFamily" in style:
        result["fontFamily"] = style["fontFamily"]
    if "fontSize" in style:
        result["fontSize"] = style["fontSize"]
    if "fontWeight" in style:
        fw = style["fontWeight"]
        if fw == "bold":
            result["fontWeight"] = 700
        elif fw == "normal":
            result["fontWeight"] = 400
        else:
            result["fontWeight"] = fw
    if "fontStyle" in style:
        result["fontStyle"] = _FONT_STYLE_MAP.get(style["fontStyle"], style["fontStyle"])
    if "lineHeight" in style:
        result["lineHeight"] = style["lineHeight"]
    if "textAlign" in style:
        result["textAlign"] = _TEXT_ALIGN_MAP.get(style["textAlign"], style["textAlign"])
    if "letterSpacing" in style:
        result["letterSpacing"] = style["letterSpacing"]
    if "textDecoration" in style:
        result["textDecoration"] = _TEXT_DECORATION_MAP.get(style["textDecoration"], style["textDecoration"])
    if "textTransform" in style:
        result["textTransform"] = _TEXT_TRANSFORM_MAP.get(style["textTransform"], style["textTransform"])
    if "hyphens" in style:
        result["hyphens"] = _HYPHENS_MAP.get(style["hyphens"], style["hyphens"])
    if "lang" in style:
        result["lang"] = style["lang"]
    if "direction" in style:
        result["direction"] = style["direction"]
    if "textOverflow" in style:
        result["textOverflow"] = _TEXT_OVERFLOW_MAP.get(style["textOverflow"], style["textOverflow"])
    if "lineBreaking" in style:
        result["lineBreaking"] = _LINE_BREAKING_MAP.get(style["lineBreaking"], style["lineBreaking"])
    if "overflow" in style:
        result["overflow"] = _OVERFLOW_MAP.get(style["overflow"], style["overflow"])

    # Color
    if "color" in style:
        result["color"] = _parse_color(style["color"])
    if "backgroundColor" in style:
        result["backgroundColor"] = _parse_color(style["backgroundColor"])
    if "opacity" in style:
        result["opacity"] = style["opacity"]

    # Border — cascade: border < borderTop/... < borderWidth/borderColor < borderTopWidth/...
    short_width: Dict[str, Optional[float]] = {"top": None, "right": None, "bottom": None, "left": None}
    short_color: Dict[str, Optional[Dict[str, float]]] = {"top": None, "right": None, "bottom": None, "left": None}

    if "border" in style:
        w, c = _parse_border_string(style["border"])
        if w is not None:
            short_width = {"top": w, "right": w, "bottom": w, "left": w}
        if c is not None:
            short_color = {"top": c, "right": c, "bottom": c, "left": c}

    for side, prop in [("top", "borderTop"), ("right", "borderRight"), ("bottom", "borderBottom"), ("left", "borderLeft")]:
        val = style.get(prop)
        if val is None:
            continue
        if isinstance(val, (int, float)):
            short_width[side] = val
        else:
            w, c = _parse_border_string(val)
            if w is not None:
                short_width[side] = w
            if c is not None:
                short_color[side] = c

    has_border_width = any(k in style for k in ("borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth"))
    has_short_width = any(v is not None for v in short_width.values())
    if has_border_width or has_short_width:
        if "borderWidth" in style:
            bw = style["borderWidth"]
            if isinstance(bw, (int, float)):
                base_bw = {"top": bw, "right": bw, "bottom": bw, "left": bw}
            else:
                base_bw = bw
        else:
            base_bw = {
                "top": short_width["top"] or 0,
                "right": short_width["right"] or 0,
                "bottom": short_width["bottom"] or 0,
                "left": short_width["left"] or 0,
            }
        result["borderWidth"] = {
            "top": style.get("borderTopWidth", base_bw["top"]),
            "right": style.get("borderRightWidth", base_bw["right"]),
            "bottom": style.get("borderBottomWidth", base_bw["bottom"]),
            "left": style.get("borderLeftWidth", base_bw["left"]),
        }

    has_border_color = any(k in style for k in ("borderColor", "borderTopColor", "borderRightColor", "borderBottomColor", "borderLeftColor"))
    has_short_color = any(v is not None for v in short_color.values())
    if has_border_color or has_short_color:
        default_c = _parse_color("#000000")
        base_bc = {
            "top": short_color["top"] or default_c,
            "right": short_color["right"] or default_c,
            "bottom": short_color["bottom"] or default_c,
            "left": short_color["left"] or default_c,
        }
        if "borderColor" in style:
            bc = style["borderColor"]
            if isinstance(bc, str):
                c = _parse_color(bc)
                base_bc = {"top": c, "right": c, "bottom": c, "left": c}
            elif isinstance(bc, dict):
                base_bc = {
                    "top": _parse_color(bc["top"]),
                    "right": _parse_color(bc["right"]),
                    "bottom": _parse_color(bc["bottom"]),
                    "left": _parse_color(bc["left"]),
                }
        result["borderColor"] = {
            "top": _parse_color(style["borderTopColor"]) if "borderTopColor" in style else base_bc["top"],
            "right": _parse_color(style["borderRightColor"]) if "borderRightColor" in style else base_bc["right"],
            "bottom": _parse_color(style["borderBottomColor"]) if "borderBottomColor" in style else base_bc["bottom"],
            "left": _parse_color(style["borderLeftColor"]) if "borderLeftColor" in style else base_bc["left"],
        }

    # Border radius
    _br_keys = ("borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius")
    if any(k in style for k in _br_keys):
        base_br = _expand_corners(style["borderRadius"]) if "borderRadius" in style else {"top_left": 0, "top_right": 0, "bottom_right": 0, "bottom_left": 0}
        result["borderRadius"] = {
            "top_left": style.get("borderTopLeftRadius", base_br["top_left"]),
            "top_right": style.get("borderTopRightRadius", base_br["top_right"]),
            "bottom_right": style.get("borderBottomRightRadius", base_br["bottom_right"]),
            "bottom_left": style.get("borderBottomLeftRadius", base_br["bottom_left"]),
        }

    # Positioning
    if "position" in style:
        result["position"] = _POSITION_MAP.get(style["position"], style["position"])
    for key in ("top", "right", "bottom", "left"):
        if key in style:
            result[key] = style[key]

    # Page behavior
    for key in ("wrap", "breakBefore", "minWidowLines", "minOrphanLines"):
        if key in style:
            result[key] = style[key]

    return result


def _map_grid_track(track: Any) -> Any:
    """Convert a grid track size to Forme JSON format."""
    if isinstance(track, (int, float)):
        return {"Pt": track}
    if track == "auto":
        return "Auto"
    if isinstance(track, str):
        m = re.match(r"^([0-9.]+)fr$", track)
        if m:
            return {"Fr": float(m.group(1))}
        try:
            return {"Pt": float(track)}
        except ValueError:
            return "Auto"
    if isinstance(track, dict) and "min" in track and "max" in track:
        return {"MinMax": [_map_grid_track(track["min"]), _map_grid_track(track["max"])]}
    return "Auto"


def _expand_repeat(s: str) -> str:
    """Expand repeat(N, tracks) in a grid template string."""
    def _replacer(m: re.Match) -> str:
        count = int(m.group(1))
        tracks = m.group(2).strip()
        return " ".join([tracks] * count)
    return re.sub(r"repeat\(\s*(\d+)\s*,\s*([^)]+)\)", _replacer, s)


def _parse_grid_template(value: Any) -> list:
    """Parse a grid template string or list to Forme JSON format."""
    if isinstance(value, list):
        return [_map_grid_track(t) for t in value]
    expanded = _expand_repeat(str(value))
    return [_map_grid_track(t) for t in expanded.split() if t]


# ─── Components ──────────────────────────────────────────────────────


class _Component:
    """Base class for all Forme components."""

    def to_dict(self) -> Dict[str, Any]:
        raise NotImplementedError


class PageBreak(_Component):
    """Force a page break."""

    def to_dict(self) -> Dict[str, Any]:
        return {"kind": {"type": "PageBreak"}, "style": {}, "children": []}


class Text(_Component):
    """A text node, optionally with inline styled runs."""

    def __init__(
        self,
        content: str = "",
        *,
        style: Optional[Dict[str, Any]] = None,
        href: Optional[str] = None,
        children: Optional[List[Any]] = None,
    ):
        self.content = content
        self.style = style
        self.href = href
        self.children = children  # List of TextRun dicts or Text components

    def to_dict(self) -> Dict[str, Any]:
        kind: Dict[str, Any] = {"type": "Text", "content": self.content}

        if self.children:
            runs = []
            for child in self.children:
                if isinstance(child, str):
                    runs.append({"content": child})
                elif isinstance(child, Text):
                    run: Dict[str, Any] = {"content": child.content}
                    if child.style:
                        run["style"] = _map_style(child.style)
                    if child.href:
                        run["href"] = child.href
                    runs.append(run)
                elif isinstance(child, dict):
                    runs.append(child)
            kind["runs"] = runs

        node: Dict[str, Any] = {
            "kind": kind,
            "style": _map_style(self.style),
            "children": [],
        }
        if self.href:
            node["href"] = self.href
        return node


class View(_Component):
    """A flex/grid container."""

    def __init__(
        self,
        *children: _Component,
        style: Optional[Dict[str, Any]] = None,
        wrap: Optional[bool] = None,
        bookmark: Optional[str] = None,
        href: Optional[str] = None,
    ):
        self.children = list(children)
        self.style = style
        self.wrap = wrap
        self.bookmark = bookmark
        self.href = href

    def to_dict(self) -> Dict[str, Any]:
        mapped = _map_style(self.style)
        if self.wrap is not None:
            mapped["wrap"] = self.wrap
        node: Dict[str, Any] = {
            "kind": {"type": "View"},
            "style": mapped,
            "children": [c.to_dict() for c in self.children],
        }
        if self.bookmark:
            node["bookmark"] = self.bookmark
        if self.href:
            node["href"] = self.href
        return node


class Image(_Component):
    """An image node (JPEG, PNG, WebP, or data URI)."""

    def __init__(
        self,
        src: str,
        *,
        width: Optional[float] = None,
        height: Optional[float] = None,
        style: Optional[Dict[str, Any]] = None,
        href: Optional[str] = None,
        alt: Optional[str] = None,
    ):
        self.src = src
        self.width = width
        self.height = height
        self.style = style
        self.href = href
        self.alt = alt

    def to_dict(self) -> Dict[str, Any]:
        kind: Dict[str, Any] = {"type": "Image", "src": self.src}
        if self.width is not None:
            kind["width"] = self.width
        if self.height is not None:
            kind["height"] = self.height
        node: Dict[str, Any] = {
            "kind": kind,
            "style": _map_style(self.style),
            "children": [],
        }
        if self.href:
            node["href"] = self.href
        if self.alt:
            node["alt"] = self.alt
        return node


class Cell(_Component):
    """A table cell."""

    def __init__(
        self,
        *children: _Component,
        col_span: Optional[int] = None,
        row_span: Optional[int] = None,
        style: Optional[Dict[str, Any]] = None,
    ):
        self.children = list(children)
        self.col_span = col_span
        self.row_span = row_span
        self.style = style

    def to_dict(self) -> Dict[str, Any]:
        kind: Dict[str, Any] = {"type": "TableCell"}
        if self.col_span is not None:
            kind["col_span"] = self.col_span
        if self.row_span is not None:
            kind["row_span"] = self.row_span
        return {
            "kind": kind,
            "style": _map_style(self.style),
            "children": [c.to_dict() for c in self.children],
        }


class Row(_Component):
    """A table row."""

    def __init__(
        self,
        *children: _Component,
        header: bool = False,
        style: Optional[Dict[str, Any]] = None,
    ):
        self.children = list(children)
        self.header = header
        self.style = style

    def to_dict(self) -> Dict[str, Any]:
        return {
            "kind": {"type": "TableRow", "is_header": self.header},
            "style": _map_style(self.style),
            "children": [c.to_dict() for c in self.children],
        }


class Table(_Component):
    """A table with column definitions."""

    def __init__(
        self,
        *children: _Component,
        columns: Optional[List[Dict[str, Any]]] = None,
        style: Optional[Dict[str, Any]] = None,
    ):
        self.children = list(children)
        self.columns = columns
        self.style = style

    def to_dict(self) -> Dict[str, Any]:
        cols = []
        if self.columns:
            for col in self.columns:
                w = col.get("width")
                if isinstance(w, (int, float)):
                    mapped_w = {"Fixed": w}
                elif isinstance(w, str) and w.endswith("fr"):
                    mapped_w = {"Fr": float(w[:-2])}
                elif w == "auto":
                    mapped_w = "Auto"
                else:
                    mapped_w = "Auto"
                cols.append({"width": mapped_w})
        return {
            "kind": {"type": "Table", "columns": cols},
            "style": _map_style(self.style),
            "children": [c.to_dict() for c in self.children],
        }


class Svg(_Component):
    """An SVG element."""

    def __init__(
        self,
        width: float,
        height: float,
        content: str,
        *,
        view_box: Optional[str] = None,
        style: Optional[Dict[str, Any]] = None,
    ):
        self.width = width
        self.height = height
        self.content = content
        self.view_box = view_box
        self.style = style

    def to_dict(self) -> Dict[str, Any]:
        kind: Dict[str, Any] = {
            "type": "Svg",
            "width": self.width,
            "height": self.height,
            "content": self.content,
        }
        if self.view_box:
            kind["view_box"] = self.view_box
        return {
            "kind": kind,
            "style": _map_style(self.style),
            "children": [],
        }


class QrCode(_Component):
    """A QR code element."""

    def __init__(
        self,
        data: str,
        *,
        size: Optional[float] = None,
        color: Optional[str] = None,
        style: Optional[Dict[str, Any]] = None,
    ):
        self.data = data
        self.size = size
        self.color = color
        self.style = style

    def to_dict(self) -> Dict[str, Any]:
        kind: Dict[str, Any] = {"type": "QrCode", "data": self.data}
        if self.size is not None:
            kind["size"] = self.size
        if self.color:
            kind["color"] = _parse_color(self.color)
        return {
            "kind": kind,
            "style": _map_style(self.style),
            "children": [],
        }


class Barcode(_Component):
    """A 1D barcode element."""

    def __init__(
        self,
        data: str,
        *,
        format: str = "Code128",
        width: Optional[float] = None,
        height: float = 60.0,
        color: Optional[str] = None,
        style: Optional[Dict[str, Any]] = None,
    ):
        self.data = data
        self.format = format
        self.width = width
        self.height = height
        self.color = color
        self.style = style

    def to_dict(self) -> Dict[str, Any]:
        kind: Dict[str, Any] = {
            "type": "Barcode",
            "data": self.data,
            "format": self.format,
            "height": self.height,
        }
        if self.width is not None:
            kind["width"] = self.width
        style = _map_style(self.style)
        if self.color:
            style["color"] = _parse_color(self.color)
        return {
            "kind": kind,
            "style": style,
            "children": [],
        }


class Fixed(_Component):
    """A fixed-position element (header/footer)."""

    def __init__(
        self,
        *children: _Component,
        position: str = "top",
        style: Optional[Dict[str, Any]] = None,
    ):
        self.children = list(children)
        self.position = position
        self.style = style

    def to_dict(self) -> Dict[str, Any]:
        pos_map = {"top": "Top", "bottom": "Bottom"}
        return {
            "kind": {"type": "Fixed", "position": pos_map.get(self.position, "Top")},
            "style": _map_style(self.style),
            "children": [c.to_dict() for c in self.children],
        }


class Watermark(_Component):
    """A watermark rendered on every page."""

    def __init__(
        self,
        text: str,
        *,
        font_size: Optional[float] = None,
        color: Optional[str] = None,
        angle: Optional[float] = None,
        style: Optional[Dict[str, Any]] = None,
    ):
        self.text = text
        self.font_size = font_size
        self.color = color
        self.angle = angle
        self.style = style

    def to_dict(self) -> Dict[str, Any]:
        kind: Dict[str, Any] = {"type": "Watermark", "text": self.text}
        if self.font_size is not None:
            kind["font_size"] = self.font_size
        if self.color:
            kind["color"] = _parse_color(self.color)
        if self.angle is not None:
            kind["angle"] = self.angle
        return {
            "kind": kind,
            "style": _map_style(self.style),
            "children": [],
        }


class BarChart(_Component):
    """A bar chart rendered as native vector graphics."""

    def __init__(
        self,
        data: list,
        *,
        width: float = 400.0,
        height: float = 200.0,
        color: Optional[str] = None,
        show_labels: bool = True,
        show_values: bool = False,
        show_grid: bool = False,
        title: Optional[str] = None,
        style: Optional[Dict[str, Any]] = None,
    ):
        self.data = data
        self.width = width
        self.height = height
        self.color = color
        self.show_labels = show_labels
        self.show_values = show_values
        self.show_grid = show_grid
        self.title = title
        self.style = style

    def to_dict(self) -> Dict[str, Any]:
        kind: Dict[str, Any] = {
            "type": "BarChart",
            "data": self.data,
            "width": self.width,
            "height": self.height,
            "show_labels": self.show_labels,
            "show_values": self.show_values,
            "show_grid": self.show_grid,
        }
        if self.color is not None:
            kind["color"] = self.color
        if self.title is not None:
            kind["title"] = self.title
        return {
            "kind": kind,
            "style": _map_style(self.style),
            "children": [],
        }


class LineChart(_Component):
    """A line chart rendered as native vector graphics."""

    def __init__(
        self,
        series: list,
        labels: list,
        *,
        width: float = 400.0,
        height: float = 200.0,
        show_points: bool = False,
        show_grid: bool = False,
        title: Optional[str] = None,
        style: Optional[Dict[str, Any]] = None,
    ):
        self.series = series
        self.labels = labels
        self.width = width
        self.height = height
        self.show_points = show_points
        self.show_grid = show_grid
        self.title = title
        self.style = style

    def to_dict(self) -> Dict[str, Any]:
        kind: Dict[str, Any] = {
            "type": "LineChart",
            "series": self.series,
            "labels": self.labels,
            "width": self.width,
            "height": self.height,
            "show_points": self.show_points,
            "show_grid": self.show_grid,
        }
        if self.title is not None:
            kind["title"] = self.title
        return {
            "kind": kind,
            "style": _map_style(self.style),
            "children": [],
        }


class PieChart(_Component):
    """A pie/donut chart rendered as native vector graphics."""

    def __init__(
        self,
        data: list,
        *,
        width: float = 200.0,
        height: float = 200.0,
        donut: bool = False,
        show_legend: bool = False,
        title: Optional[str] = None,
        style: Optional[Dict[str, Any]] = None,
    ):
        self.data = data
        self.width = width
        self.height = height
        self.donut = donut
        self.show_legend = show_legend
        self.title = title
        self.style = style

    def to_dict(self) -> Dict[str, Any]:
        kind: Dict[str, Any] = {
            "type": "PieChart",
            "data": self.data,
            "width": self.width,
            "height": self.height,
            "donut": self.donut,
            "show_legend": self.show_legend,
        }
        if self.title is not None:
            kind["title"] = self.title
        return {
            "kind": kind,
            "style": _map_style(self.style),
            "children": [],
        }


class AreaChart(_Component):
    """An area chart rendered as native vector graphics."""

    def __init__(
        self,
        series: list,
        labels: list,
        *,
        width: float = 400.0,
        height: float = 200.0,
        show_grid: bool = False,
        title: Optional[str] = None,
        style: Optional[Dict[str, Any]] = None,
    ):
        self.series = series
        self.labels = labels
        self.width = width
        self.height = height
        self.show_grid = show_grid
        self.title = title
        self.style = style

    def to_dict(self) -> Dict[str, Any]:
        kind: Dict[str, Any] = {
            "type": "AreaChart",
            "series": self.series,
            "labels": self.labels,
            "width": self.width,
            "height": self.height,
            "show_grid": self.show_grid,
        }
        if self.title is not None:
            kind["title"] = self.title
        return {
            "kind": kind,
            "style": _map_style(self.style),
            "children": [],
        }


class DotPlot(_Component):
    """A dot plot (scatter plot) rendered as native vector graphics."""

    def __init__(
        self,
        groups: list,
        *,
        width: float = 400.0,
        height: float = 300.0,
        x_min: Optional[float] = None,
        x_max: Optional[float] = None,
        y_min: Optional[float] = None,
        y_max: Optional[float] = None,
        x_label: Optional[str] = None,
        y_label: Optional[str] = None,
        show_legend: bool = False,
        dot_size: float = 4.0,
        style: Optional[Dict[str, Any]] = None,
    ):
        self.groups = groups
        self.width = width
        self.height = height
        self.x_min = x_min
        self.x_max = x_max
        self.y_min = y_min
        self.y_max = y_max
        self.x_label = x_label
        self.y_label = y_label
        self.show_legend = show_legend
        self.dot_size = dot_size
        self.style = style

    def to_dict(self) -> Dict[str, Any]:
        kind: Dict[str, Any] = {
            "type": "DotPlot",
            "groups": self.groups,
            "width": self.width,
            "height": self.height,
            "show_legend": self.show_legend,
            "dot_size": self.dot_size,
        }
        if self.x_min is not None:
            kind["x_min"] = self.x_min
        if self.x_max is not None:
            kind["x_max"] = self.x_max
        if self.y_min is not None:
            kind["y_min"] = self.y_min
        if self.y_max is not None:
            kind["y_max"] = self.y_max
        if self.x_label is not None:
            kind["x_label"] = self.x_label
        if self.y_label is not None:
            kind["y_label"] = self.y_label
        return {
            "kind": kind,
            "style": _map_style(self.style),
            "children": [],
        }


class TextField(_Component):
    """An interactive text input field (PDF AcroForm widget)."""

    def __init__(
        self,
        name: str,
        *,
        value: Optional[str] = None,
        placeholder: Optional[str] = None,
        width: float = 200.0,
        height: float = 24.0,
        multiline: bool = False,
        password: bool = False,
        read_only: bool = False,
        max_length: Optional[int] = None,
        font_size: float = 12.0,
        style: Optional[Dict[str, Any]] = None,
    ):
        self.name = name
        self.value = value
        self.placeholder = placeholder
        self.width = width
        self.height = height
        self.multiline = multiline
        self.password = password
        self.read_only = read_only
        self.max_length = max_length
        self.font_size = font_size
        self.style = style

    def to_dict(self) -> Dict[str, Any]:
        kind: Dict[str, Any] = {
            "type": "TextField",
            "name": self.name,
            "width": self.width,
            "height": self.height,
            "multiline": self.multiline,
            "password": self.password,
            "read_only": self.read_only,
            "font_size": self.font_size,
        }
        if self.value is not None:
            kind["value"] = self.value
        if self.placeholder is not None:
            kind["placeholder"] = self.placeholder
        if self.max_length is not None:
            kind["max_length"] = self.max_length
        return {
            "kind": kind,
            "style": _map_style(self.style),
            "children": [],
        }


class Checkbox(_Component):
    """An interactive checkbox (PDF AcroForm widget)."""

    def __init__(
        self,
        name: str,
        *,
        checked: bool = False,
        width: float = 14.0,
        height: float = 14.0,
        read_only: bool = False,
        style: Optional[Dict[str, Any]] = None,
    ):
        self.name = name
        self.checked = checked
        self.width = width
        self.height = height
        self.read_only = read_only
        self.style = style

    def to_dict(self) -> Dict[str, Any]:
        return {
            "kind": {
                "type": "Checkbox",
                "name": self.name,
                "checked": self.checked,
                "width": self.width,
                "height": self.height,
                "read_only": self.read_only,
            },
            "style": _map_style(self.style),
            "children": [],
        }


class Dropdown(_Component):
    """An interactive dropdown / combo box (PDF AcroForm widget)."""

    def __init__(
        self,
        name: str,
        options: List[str],
        *,
        value: Optional[str] = None,
        width: float = 200.0,
        height: float = 24.0,
        read_only: bool = False,
        font_size: float = 12.0,
        style: Optional[Dict[str, Any]] = None,
    ):
        self.name = name
        self.options = options
        self.value = value
        self.width = width
        self.height = height
        self.read_only = read_only
        self.font_size = font_size
        self.style = style

    def to_dict(self) -> Dict[str, Any]:
        kind: Dict[str, Any] = {
            "type": "Dropdown",
            "name": self.name,
            "options": self.options,
            "width": self.width,
            "height": self.height,
            "read_only": self.read_only,
            "font_size": self.font_size,
        }
        if self.value is not None:
            kind["value"] = self.value
        return {
            "kind": kind,
            "style": _map_style(self.style),
            "children": [],
        }


class RadioButton(_Component):
    """An interactive radio button (PDF AcroForm widget).

    Multiple RadioButtons with the same ``name`` form a mutually exclusive group.
    """

    def __init__(
        self,
        name: str,
        value: str,
        *,
        checked: bool = False,
        width: float = 14.0,
        height: float = 14.0,
        read_only: bool = False,
        style: Optional[Dict[str, Any]] = None,
    ):
        self.name = name
        self.value = value
        self.checked = checked
        self.width = width
        self.height = height
        self.read_only = read_only
        self.style = style

    def to_dict(self) -> Dict[str, Any]:
        return {
            "kind": {
                "type": "RadioButton",
                "name": self.name,
                "value": self.value,
                "checked": self.checked,
                "width": self.width,
                "height": self.height,
                "read_only": self.read_only,
            },
            "style": _map_style(self.style),
            "children": [],
        }


class Page(_Component):
    """A page with size and margin configuration."""

    _PAGE_SIZES = {"A4", "Letter", "Legal", "A3", "A5", "B4", "B5", "Tabloid"}

    def __init__(
        self,
        *children: _Component,
        size: Optional[Any] = None,
        margin: Optional[Any] = None,
    ):
        self.children = list(children)
        self.size = size
        self.margin = margin

    def to_dict(self) -> Dict[str, Any]:
        # Size
        if self.size is None:
            page_size: Any = "A4"
        elif isinstance(self.size, str):
            page_size = self.size
        elif isinstance(self.size, dict):
            page_size = {"Custom": {"width": self.size["width"], "height": self.size["height"]}}
        else:
            page_size = "A4"

        # Margin
        if self.margin is None:
            page_margin = {"top": 54, "right": 54, "bottom": 54, "left": 54}
        else:
            page_margin = _expand_edges(self.margin)

        config = {"size": page_size, "margin": page_margin, "wrap": True}
        return {
            "kind": {"type": "Page", "config": config},
            "style": {},
            "children": [c.to_dict() for c in self.children],
        }


class Document(_Component):
    """Root document node. Can render to PDF bytes or serialize to JSON."""

    def __init__(
        self,
        *children: _Component,
        title: Optional[str] = None,
        author: Optional[str] = None,
        subject: Optional[str] = None,
        lang: Optional[str] = None,
        style: Optional[Dict[str, Any]] = None,
        fonts: Optional[List[Dict[str, Any]]] = None,
        tagged: bool = False,
        pdfa: Optional[str] = None,
        pdf_ua: bool = False,
        embed_standard_fonts: bool = False,
        certification: Optional[Dict[str, Any]] = None,
        signature: Optional[Dict[str, Any]] = None,
    ):
        self.children = list(children)
        self.title = title
        self.author = author
        self.subject = subject
        self.lang = lang
        self.style = style
        self.fonts = fonts
        self.tagged = tagged
        self.pdfa = pdfa
        self.pdf_ua = pdf_ua
        self.embed_standard_fonts = embed_standard_fonts
        if signature is not None and certification is None:
            import warnings
            warnings.warn("signature= is deprecated, use certification=", DeprecationWarning, stacklevel=2)
            certification = signature
        self.certification = certification

    def to_dict(self) -> Dict[str, Any]:
        doc: Dict[str, Any] = {
            "children": [c.to_dict() for c in self.children],
        }

        metadata: Dict[str, Any] = {}
        if self.title:
            metadata["title"] = self.title
        if self.author:
            metadata["author"] = self.author
        if self.subject:
            metadata["subject"] = self.subject
        if self.lang:
            metadata["lang"] = self.lang
        if metadata:
            doc["metadata"] = metadata

        if self.style:
            doc["default_style"] = _map_style(self.style)

        if self.fonts:
            doc["fonts"] = self.fonts

        if self.tagged:
            doc["tagged"] = True

        if self.pdfa:
            doc["pdfa"] = self.pdfa

        if self.pdf_ua:
            doc["pdfUa"] = True

        if self.embed_standard_fonts:
            doc["embedStandardFonts"] = True

        if self.certification:
            doc["certification"] = self.certification

        return doc

    def to_json(self, **kwargs: Any) -> str:
        """Serialize to a JSON string."""
        return json.dumps(self.to_dict(), **kwargs)

    def render(self, *, embed_data: Any = None, flatten_forms: bool = False) -> bytes:
        """Render to PDF bytes using the local WASM engine.

        Args:
            embed_data: Optional data to embed in the PDF as a JSON attachment.
            flatten_forms: When True, form fields are rendered as static text.

        Returns:
            Raw PDF file bytes.

        Raises:
            FormeRenderError: If the engine returns an error.
            ImportError: If wasmtime is not installed.
        """
        from .wasm import render_pdf

        doc = self.to_dict()
        if embed_data is not None:
            doc["embedded_data"] = json.dumps(embed_data)
        if flatten_forms:
            doc["flattenForms"] = True
        return render_pdf(json.dumps(doc))


def certify(
    pdf: bytes,
    certificate: str,
    private_key: str,
    reason: Optional[str] = None,
    location: Optional[str] = None,
    contact: Optional[str] = None,
) -> bytes:
    """Certify PDF bytes with an X.509 certificate.

    Args:
        pdf: Raw PDF file bytes.
        certificate: PEM-encoded X.509 certificate.
        private_key: PEM-encoded RSA private key (PKCS#8).
        reason: Optional reason for certification.
        location: Optional location of certification.
        contact: Optional contact info for the certifier.

    Returns:
        Certified PDF file bytes.
    """
    from .wasm import certify_pdf

    config = {
        "certificatePem": certificate,
        "privateKeyPem": private_key,
    }
    if reason is not None:
        config["reason"] = reason
    if location is not None:
        config["location"] = location
    if contact is not None:
        config["contact"] = contact
    return certify_pdf(pdf, json.dumps(config))


def sign(
    pdf: bytes,
    certificate: str,
    private_key: str,
    reason: Optional[str] = None,
    location: Optional[str] = None,
    contact: Optional[str] = None,
) -> bytes:
    """Sign PDF bytes with an X.509 certificate.

    .. deprecated:: Use certify instead.
    """
    import warnings
    warnings.warn("sign() is deprecated, use certify()", DeprecationWarning, stacklevel=2)
    return certify(pdf, certificate, private_key, reason, location, contact)
