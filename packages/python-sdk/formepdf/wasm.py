"""WASM bridge for local PDF rendering via wasmtime.

Requires: pip install formepdf[local]  (adds wasmtime dependency)
"""
from __future__ import annotations

import ctypes
import os
from pathlib import Path
from typing import Optional

_WASM_PATH = Path(__file__).parent / "forme.wasm"

# Lazy singleton
_engine: Optional["_FormeEngine"] = None


class FormeRenderError(Exception):
    """Raised when the Forme WASM engine returns an error."""
    pass


class _FormeEngine:
    """Manages a wasmtime instance with the Forme WASM module loaded."""

    def __init__(self, wasm_path: Path) -> None:
        try:
            import wasmtime  # type: ignore[import-untyped]
        except ImportError:
            raise ImportError(
                "wasmtime is required for local rendering. "
                "Install it with: pip install formepdf[local]"
            ) from None

        self._wasmtime = wasmtime
        engine = wasmtime.Engine()
        module = wasmtime.Module.from_file(engine, str(wasm_path))

        linker = wasmtime.Linker(engine)
        linker.define_wasi()

        wasi_config = wasmtime.WasiConfig()
        wasi_config.inherit_stdout()
        wasi_config.inherit_stderr()

        self._store = wasmtime.Store(engine)
        self._store.set_wasi(wasi_config)

        self._instance = linker.instantiate(self._store, module)
        self._memory = self._instance.exports(self._store)["memory"]
        self._alloc = self._instance.exports(self._store)["forme_alloc"]
        self._dealloc = self._instance.exports(self._store)["forme_dealloc"]
        self._render = self._instance.exports(self._store)["forme_render_pdf"]
        self._render_html = self._instance.exports(self._store)["forme_render_html"]
        self._certify = self._instance.exports(self._store)["forme_certify_pdf"]
        self._result_ptr = self._instance.exports(self._store)["forme_get_result_ptr"]
        self._result_len = self._instance.exports(self._store)["forme_get_result_len"]
        self._error_ptr = self._instance.exports(self._store)["forme_get_error_ptr"]
        self._error_len = self._instance.exports(self._store)["forme_get_error_len"]
        self._free_result = self._instance.exports(self._store)["forme_free_result"]

    def _read_memory(self, ptr: int, length: int) -> bytes:
        """Read bytes from WASM linear memory."""
        data_ptr = self._memory.data_ptr(self._store)
        base = ctypes.addressof(data_ptr.contents)
        buf = (ctypes.c_ubyte * length).from_address(base + ptr)
        return bytes(buf)

    def _write_memory(self, ptr: int, data: bytes) -> None:
        """Write bytes into WASM linear memory."""
        data_ptr = self._memory.data_ptr(self._store)
        base = ctypes.addressof(data_ptr.contents)
        ctypes.memmove(base + ptr, data, len(data))

    def render_pdf(self, json_str: str) -> bytes:
        """Render a JSON document to PDF bytes."""
        json_bytes = json_str.encode("utf-8")
        length = len(json_bytes)

        # Allocate input buffer in WASM memory
        input_ptr = self._alloc(self._store, length, 1)
        if not input_ptr:
            raise FormeRenderError("Failed to allocate WASM memory for input")

        try:
            # Write JSON into WASM memory
            self._write_memory(input_ptr, json_bytes)

            # Call render
            status = self._render(self._store, input_ptr, length)

            if status != 0:
                # Read error
                err_ptr = self._error_ptr(self._store)
                err_len = self._error_len(self._store)
                if err_ptr and err_len > 0:
                    error_msg = self._read_memory(err_ptr, err_len).decode("utf-8")
                else:
                    error_msg = "Unknown render error"
                raise FormeRenderError(error_msg)

            # Read result
            res_ptr = self._result_ptr(self._store)
            res_len = self._result_len(self._store)
            if not res_ptr or res_len == 0:
                raise FormeRenderError("Render returned empty result")

            pdf_bytes = self._read_memory(res_ptr, res_len)

            # Free the result buffer in WASM
            self._free_result(self._store)

            return pdf_bytes
        finally:
            # Free input buffer
            self._dealloc(self._store, input_ptr, length, 1)


    def render_html(self, html_str: str) -> bytes:
        """Render an HTML + print-CSS string to PDF bytes (default options).

        Byte-identical to `@formepdf/html`'s `renderHtml(html, {})` — same
        engine, run as wasm32-wasip1.
        """
        html_bytes = html_str.encode("utf-8")
        length = len(html_bytes)

        input_ptr = self._alloc(self._store, length, 1)
        if not input_ptr:
            raise FormeRenderError("Failed to allocate WASM memory for input")

        try:
            self._write_memory(input_ptr, html_bytes)

            status = self._render_html(self._store, input_ptr, length)

            if status != 0:
                err_ptr = self._error_ptr(self._store)
                err_len = self._error_len(self._store)
                if err_ptr and err_len > 0:
                    error_msg = self._read_memory(err_ptr, err_len).decode("utf-8")
                else:
                    error_msg = "Unknown render error"
                raise FormeRenderError(error_msg)

            res_ptr = self._result_ptr(self._store)
            res_len = self._result_len(self._store)
            if not res_ptr or res_len == 0:
                raise FormeRenderError("Render returned empty result")

            pdf_bytes = self._read_memory(res_ptr, res_len)
            self._free_result(self._store)
            return pdf_bytes
        finally:
            self._dealloc(self._store, input_ptr, length, 1)


    def certify_pdf(self, pdf_bytes: bytes, config_json: str) -> bytes:
        """Certify PDF bytes with an X.509 certificate."""
        config_bytes = config_json.encode("utf-8")

        # Allocate input buffers in WASM memory
        pdf_ptr = self._alloc(self._store, len(pdf_bytes), 1)
        config_ptr = self._alloc(self._store, len(config_bytes), 1)
        if not pdf_ptr or not config_ptr:
            raise FormeRenderError("Failed to allocate WASM memory for certify input")

        try:
            self._write_memory(pdf_ptr, pdf_bytes)
            self._write_memory(config_ptr, config_bytes)

            status = self._certify(
                self._store, pdf_ptr, len(pdf_bytes), config_ptr, len(config_bytes)
            )

            if status != 0:
                err_ptr = self._error_ptr(self._store)
                err_len = self._error_len(self._store)
                if err_ptr and err_len > 0:
                    error_msg = self._read_memory(err_ptr, err_len).decode("utf-8")
                else:
                    error_msg = "Unknown certify error"
                raise FormeRenderError(error_msg)

            res_ptr = self._result_ptr(self._store)
            res_len = self._result_len(self._store)
            if not res_ptr or res_len == 0:
                raise FormeRenderError("Certify returned empty result")

            certified_bytes = self._read_memory(res_ptr, res_len)
            self._free_result(self._store)
            return certified_bytes
        finally:
            self._dealloc(self._store, pdf_ptr, len(pdf_bytes), 1)
            self._dealloc(self._store, config_ptr, len(config_bytes), 1)


def _get_engine() -> _FormeEngine:
    """Get or create the singleton engine instance."""
    global _engine
    if _engine is None:
        if not _WASM_PATH.exists():
            raise FileNotFoundError(
                f"WASM binary not found at {_WASM_PATH}. "
                "Run 'bash build_wasm.sh' in the python-sdk directory, "
                "or install a pre-built wheel."
            )
        _engine = _FormeEngine(_WASM_PATH)
    return _engine


def render_pdf(json_str: str) -> bytes:
    """Render a JSON document string to PDF bytes.

    Lazily initializes the WASM engine on first call.

    Args:
        json_str: JSON string matching the Forme document schema.

    Returns:
        Raw PDF file bytes.

    Raises:
        FormeRenderError: If the engine returns an error.
        ImportError: If wasmtime is not installed.
        FileNotFoundError: If the WASM binary is not found.
    """
    return _get_engine().render_pdf(json_str)


def render_html(html_str: str) -> bytes:
    """Render an HTML + print-CSS string to PDF bytes locally.

    Runs the same engine as ``@formepdf/html`` — byte-identical to
    ``renderHtml(html, {})`` — as a wasm32-wasip1 module via wasmtime. No
    browser, no system libraries.

    Args:
        html_str: An HTML document (with optional print CSS: ``@page``,
            page counters, break control, etc.).

    Returns:
        Raw PDF file bytes.

    Raises:
        FormeRenderError: If the engine returns an error.
        ImportError: If wasmtime is not installed.
        FileNotFoundError: If the WASM binary is not found.
    """
    return _get_engine().render_html(html_str)


def certify_pdf(pdf_bytes: bytes, config_json: str) -> bytes:
    """Certify PDF bytes with an X.509 certificate.

    Args:
        pdf_bytes: Raw PDF file bytes.
        config_json: JSON string with certification configuration.

    Returns:
        Certified PDF file bytes.
    """
    return _get_engine().certify_pdf(pdf_bytes, config_json)


