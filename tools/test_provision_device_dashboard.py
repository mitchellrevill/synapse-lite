import importlib.util
import hashlib
import json
from pathlib import Path
import io
import tempfile
import unittest
from PIL import Image

spec = importlib.util.spec_from_file_location('provision', Path(__file__).with_name('provision-device-dashboard.py'))
provision = importlib.util.module_from_spec(spec)
spec.loader.exec_module(provision)


class DashboardProvisionTests(unittest.TestCase):
    def setup_capture(self, root):
        catalog = root / 'catalog.json'
        catalog.write_bytes(b'[{"productId":196}]')
        relative = 'synapse/products/196/ui/196_0/PluginImages/196_0_0_dashboard3x.avif'
        image = root / relative
        image.parent.mkdir(parents=True)
        Image.new('RGBA', (6, 4), (100, 150, 200, 128)).save(image, format='AVIF', quality=100)
        data = image.read_bytes()
        report = {'catalog_sha256': hashlib.sha256(catalog.read_bytes()).hexdigest(),
            'records': [{'status': 'available', 'product_id': 196, 'edition_id': 0, 'layout_id': 0,
                'fetched_path': '/' + relative, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}]}
        (root / 'dashboard-fetched.json').write_text(json.dumps(report))
        return catalog, image, report

    def test_native_pixels_preserve_source_alpha_without_a_matte(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            catalog, image, report = self.setup_capture(root)
            output = root / 'pack'
            self.assertEqual(provision.build(root, catalog, output), 1)
            index = json.loads((output / 'index.json').read_text(encoding='utf-8'))
            raw = (output / index['images'][0]['file']).read_bytes()
            record = index['images'][0]
            self.assertEqual((output / record['source_file']).read_bytes(), image.read_bytes())
            self.assertEqual(record['source_size'], [6, 4])
            self.assertEqual(raw[:8], b'\x89PNG\r\n\x1a\n')
            with Image.open(image) as decoded:
                expected = decoded.convert('RGBA').getpixel((0, 0))
            with Image.open(io.BytesIO(raw)) as packed:
                self.assertEqual(packed.size, (6, 4))
                self.assertEqual(packed.convert('RGBA').getpixel((0, 0)), expected)
            with self.assertRaises(ValueError):
                provision.build(root, catalog, output)

    def test_large_original_is_retained_while_native_thumbnail_stays_bounded(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            catalog, image, report = self.setup_capture(root)
            Image.new('RGBA', (1200, 800), (20, 40, 60, 80)).save(image, format='AVIF')
            blob = image.read_bytes()
            report['records'][0].update(bytes=len(blob), sha256=hashlib.sha256(blob).hexdigest())
            (root / 'dashboard-fetched.json').write_text(json.dumps(report))
            provision.build(root,catalog,root/'pack')
            row = json.loads((root/'pack/index.json').read_bytes())['images'][0]
            self.assertEqual((row['width'],row['height']),(768,512))
            self.assertEqual(row['source_size'],[1200,800])
            self.assertEqual((root/'pack'/row['source_file']).read_bytes(),blob)

    def test_duplicate_conflicting_artwork_refuses(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);catalog,image,report=self.setup_capture(root)
            alternate=image.with_name(image.name.replace('3x','1x'))
            Image.new('RGBA',(6,4),(1,2,3,4)).save(alternate,format='AVIF')
            data=alternate.read_bytes()
            duplicate=dict(report['records'][0]);duplicate.update(sha256=hashlib.sha256(data).hexdigest(),bytes=len(data),fetched_path='/' + alternate.relative_to(root).as_posix())
            report['records'].append(duplicate)
            (root/'dashboard-fetched.json').write_text(json.dumps(report))
            with self.assertRaisesRegex(ValueError,'Conflicting artwork'):provision.build(root,catalog,root/'pack')

    def test_wrong_catalog_or_escaping_asset_path_is_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            catalog, image, report = self.setup_capture(root)
            report['records'][0]['fetched_path'] = '/../elsewhere.avif'
            (root / 'dashboard-fetched.json').write_text(json.dumps(report))
            with self.assertRaises(ValueError):
                provision.build(root, catalog, root / 'invalid-path')
            catalog.write_bytes(b'[]')
            with self.assertRaises(ValueError):
                provision.build(root, catalog, root / 'wrong-catalog')


if __name__ == '__main__':
    unittest.main()
