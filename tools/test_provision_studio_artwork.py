import base64
import hashlib
import json
import unittest
import importlib.util
from pathlib import Path

spec = importlib.util.spec_from_file_location('studio_artwork', Path(__file__).with_name('provision-studio-artwork.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
project = module.project


class StudioArtworkTests(unittest.TestCase):
    def fixture(self, svg=None):
        svg = svg or b'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50"><g><path class="productfill" d="M0 0H100V50Z"/><path class="characters" d="M5 5H10V10Z"/><path class="led ledkeys" d="M1 1H20V20Z"/><path class="selection" d="M1 1H20V20Z"/></g></svg>'
        manifest = json.dumps({'svg': {'svg_0_6': base64.b64encode(svg).decode()}}).encode()
        layout = {'edition_id': 0, 'layout_id': 6, 'view_box': [0, 0, 100, 50],
                  'manifest_sha256': hashlib.sha256(manifest).hexdigest(), 'source_sha256': hashlib.sha256(svg).hexdigest()}
        return manifest, layout

    def test_layers_preserve_source_paths_without_baking_leds_or_selection(self):
        layers = project(*self.fixture())
        self.assertIn(b'productfill', layers['body'])
        self.assertNotIn(b'characters', layers['body'])
        self.assertIn(b'characters', layers['legends'])
        self.assertNotIn(b'productfill', layers['legends'])
        for layer in layers.values():
            self.assertNotIn(b'ledkeys', layer)
            self.assertNotIn(b'selection', layer)

    def test_unmatched_sources_are_refused(self):
        manifest, layout = self.fixture()
        for field in ('manifest_sha256', 'source_sha256'):
            with self.assertRaises(ValueError):
                project(manifest, {**layout, field: '0' * 64})

    def test_external_content_and_untraced_elements_are_refused(self):
        for element in (b'<image href="https://example.invalid/x"/>', b'<script/>', b'<style>@import "x";</style>'):
            with self.assertRaises(ValueError):
                project(*self.fixture(b'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50">' + element + b'</svg>'))


if __name__ == '__main__':
    unittest.main()
