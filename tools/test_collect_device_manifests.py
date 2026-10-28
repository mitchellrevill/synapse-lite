import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import urllib.error

spec = importlib.util.spec_from_file_location('collector', Path(__file__).with_name('collect-device-manifests.py'))
collector = importlib.util.module_from_spec(spec)
spec.loader.exec_module(collector)


class Response(io.BytesIO):
    headers = {'ETag': 'test-version'}


class CollectionTests(unittest.TestCase):
    def test_duplicate_products_do_not_duplicate_requests_or_guess_editions(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            catalog = [{'productId': 605}, {'productId': 605}, {'productId': 196}]
            requests, todos = collector.plan(catalog, root, 'products')
            self.assertEqual(len(requests), 4)
            self.assertEqual(todos, [])
            requests, todos = collector.plan(catalog, root, 'lighting')
            self.assertEqual(requests, [])
            self.assertEqual(len(todos), 2)
            path = root / 'synapse/products/605/mw/manifest.json'
            path.parent.mkdir(parents=True)
            path.write_text(json.dumps({'editionId': [128, 0, 128], 'layoutId': [1, 2]}))
            requests, todos = collector.plan(catalog, root, 'lighting')
            self.assertEqual([r['edition_id'] for r in requests], [0, 128])
            self.assertEqual(len(todos), 1)
            self.assertEqual(requests[1]['path'], '/synapse/products/605/mw/lighting-manifests/DeviceManifest_605_128.json')

    def test_dashboard_default_role_does_not_expand_a_layout_cartesian_product(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            path = root / 'synapse/products/605/mw/manifest.json'
            path.parent.mkdir(parents=True)
            path.write_text(json.dumps({'editionId': [0, 128], 'layoutId': list(range(1, 21)),
                'icons': [{'src': '/synapse/assets/imgs/favicon/KEYBOARD.svg'}]}))
            requests, todos = collector.plan([{'productId': 605}], root, 'dashboard')
            self.assertEqual(len(requests), 2)
            self.assertEqual(todos, [])
            self.assertEqual(requests[0]['path'], '/synapse/products/605/ui/605_0/PluginImages/605_0_1_dashboard3x.avif')
            self.assertIn('physical layout unverified', requests[0]['selection'])

    def test_dashboard_image_response_must_be_an_avif_container(self):
        with tempfile.TemporaryDirectory() as temp:
            worker = collector.Collector(Path(temp))
            record = {'path': '/synapse/products/605/ui/605_0/PluginImages/605_0_1_dashboard3x.avif', 'role': 'dashboard'}
            image = b'\x00\x00\x00\x20ftypavif\x00\x00\x00\x00avif'
            with patch.object(collector.urllib.request, 'build_opener') as builder:
                builder.return_value.open.side_effect = [Response(b'<html>error</html>'), Response(image)]
                self.assertEqual(worker.collect(record)['status'], 'todo')
                self.assertEqual(worker.collect(record)['status'], 'available')

    def test_invalid_catalogue_identity_cannot_create_a_request(self):
        for value in [-1, 65536, True, '1/../../file', None]:
            with self.assertRaises(ValueError):
                collector.plan([{'productId': value}], Path('.'), 'products')

    def test_resume_validates_prior_capture_without_network_or_new_budget(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            path = root / 'synapse/products/605/mw/manifest.json'
            path.parent.mkdir(parents=True)
            path.write_bytes(b'{"editionId":[0]}')
            worker = collector.Collector(root, resume=True)
            with patch.object(collector.urllib.request, 'build_opener') as builder:
                result = worker.collect({'path': '/synapse/products/605/mw/manifest.json'})
                self.assertEqual(result['status'], 'available')
                self.assertEqual(result['source'], 'previous_snapshot_capture_not_refreshed')
                self.assertNotIn('retrieved_utc', result)
                self.assertEqual(worker.total, 0)
                builder.assert_not_called()

    def test_capture_preserves_all_raw_layouts_and_refuses_revision_overwrite(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            worker = collector.Collector(root)
            record = {'path': '/synapse/products/605/mw/manifest.json'}
            data = b'{"editionId":[0,128],"svg":{"svg_0_1":"first","svg_128_2":"second"}}'
            with patch.object(collector.urllib.request, 'build_opener') as builder:
                builder.return_value.open.side_effect = [Response(data), Response(b'{"editionId":[0]}')]
                result = worker.collect(record)
                self.assertEqual(result['status'], 'available')
                target = root / record['path'].lstrip('/')
                self.assertEqual(target.read_bytes(), data)
                self.assertEqual(worker.collect(record)['status'], 'todo')
                self.assertEqual(target.read_bytes(), data)

    def test_rate_limit_stops_next_request_and_origin_redirect_is_restricted(self):
        with tempfile.TemporaryDirectory() as temp:
            worker = collector.Collector(Path(temp))
            record = {'path': '/synapse/products/605/ui/manifest.json'}
            with patch.object(collector.urllib.request, 'build_opener') as builder:
                builder.return_value.open.side_effect = urllib.error.HTTPError(collector.ORIGIN, 429, 'rate limit', {}, None)
                self.assertEqual(worker.collect(record)['status'], 'todo')
                self.assertEqual(worker.collect(record)['status'], 'todo')
                self.assertEqual(builder.return_value.open.call_count, 1)
            with self.assertRaises(ValueError):
                collector.SameOriginRedirect().redirect_request(None, None, 302, '', {}, 'https://elsewhere.example/installer.exe')


if __name__ == '__main__':
    unittest.main()
