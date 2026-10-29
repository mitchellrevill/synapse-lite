import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
import zipfile
from PIL import Image

spec=importlib.util.spec_from_file_location('audit',Path(__file__).with_name('audit-device-assets.py'))
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)


class AuditTests(unittest.TestCase):
    def fixture(self,root):
        art,studio,capture=(root/name for name in ('art','studio','capture'))
        for path in (art,studio,capture):path.mkdir()
        catalog=root/'catalog.json';catalog.write_bytes(b'[{"productId":100},{"productId":101}]')
        (art/'catalog.json').write_bytes(catalog.read_bytes())
        png=io.BytesIO();Image.new('RGBA',(2,2),(10,20,30,80)).save(png,format='PNG')
        blob=png.getvalue();name=f'dashboard-{m.sha(blob)}.png';(art/name).write_bytes(blob)
        artwork={'product_id':100,'edition_id':2,'layout_id':0,'file':name,'width':2,'height':2,'bytes':len(blob),'sha256':m.sha(blob)}
        self.write(art/'index.json',{'format':'synapse-lite-dashboard-pack-v2','images':[artwork]})
        blob=m.encode({'product_id':100,'edition_id':2,'layout_id':9,'keys':[{'inputID':'0:0'}]})
        (studio/'studio-100-2-9.json').write_bytes(blob)
        row={'product_id':100,'edition_id':2,'layout_id':9,'file':'studio-100-2-9.json','bytes':len(blob),'sha256':m.sha(blob),'leds':1}
        self.write(studio/'index.json',{'format':'synapse-lite-studio-presets-v1','records':[row]})
        source=capture/'synapse/products/100/mw/manifest.json';source.parent.mkdir(parents=True)
        blob=m.encode({'deviceName':'Synthetic','editionId':[2,3],'layoutId':[9,10],'productTranslations':{'100_2':{}}});source.write_bytes(blob)
        self.write(capture/'products-fetched.json',{'stage':'products','catalog_sha256':m.sha(catalog.read_bytes()),'records':[{'status':'available','product_id':100,'role':'mw','fetched_path':'/synapse/products/100/mw/manifest.json','bytes':len(blob),'sha256':m.sha(blob)}]})
        (art/'personal-database.json').write_bytes(b'never package this')
        return catalog,capture,art,studio

    def write(self,path,value):path.write_bytes(m.encode(value))

    def test_exact_coverage_keeps_missing_editions_and_unknown_products_explicit(self):
        with tempfile.TemporaryDirectory() as temp:
            args=self.fixture(Path(temp));report,_=m.audit(*args)
            self.assertEqual(report['summary']['catalogue_primary_products'],2)
            self.assertEqual(report['products'][0]['studio_exact_identities'],[[2,9]])
            self.assertEqual(report['products'][0]['mw_editions_without_artwork'],[3])
            self.assertEqual(report['products'][1]['mw'],None)
            self.assertFalse(report['hardware_support_verified'])

    def test_deterministic_package_ignores_mtimes_and_unindexed_private_files(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);args=self.fixture(root);report,files=m.audit(*args)
            a,b=root/'a.zip',root/'b.zip';m.package(a,report,files)
            for path,_,_ in files.values():os.utime(path,(1000000,1000000))
            m.package(b,report,files)
            self.assertEqual(a.read_bytes(),b.read_bytes())
            with zipfile.ZipFile(a) as archive:
                self.assertEqual(set(archive.namelist()),set(files)|{'audit.json'})
                self.assertTrue(all(i.date_time==(1980,1,1,0,0,0) for i in archive.infolist()))
            with self.assertRaises(FileExistsError):m.package(a,report,files)

    def test_duplicate_identity_changed_manifest_and_tampered_payload_refuse(self):
        for mutation in ('duplicate','manifest','payload','identity','escape'):
            with self.subTest(mutation=mutation),tempfile.TemporaryDirectory() as temp:
                root=Path(temp);args=self.fixture(root);catalog,capture,art,studio=args
                if mutation=='duplicate':
                    index=json.loads((art/'index.json').read_bytes());index['images']*=2;self.write(art/'index.json',index)
                elif mutation=='manifest':(capture/'synapse/products/100/mw/manifest.json').write_bytes(b'{}')
                elif mutation=='payload':next(art.glob('*.png')).write_bytes(b'tampered')
                else:
                    index=json.loads((studio/'index.json').read_bytes())
                    index['records'][0]['file']='../outside.json' if mutation=='escape' else 'studio-101-2-9.json'
                    self.write(studio/'index.json',index)
                with self.assertRaises((ValueError,FileNotFoundError)):m.audit(*args)

    def test_changed_file_after_audit_removes_incomplete_archive(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);args=self.fixture(root);report,files=m.audit(*args)
            next(args[2].glob('*.png')).write_bytes(b'changed')
            with self.assertRaises(ValueError):m.package(root/'bad.zip',report,files)
            self.assertFalse((root/'bad.zip').exists())

    def test_retained_original_is_verified_and_tampering_refuses(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);args=self.fixture(root);art=args[2]
            raw=io.BytesIO();Image.new('RGBA',(4,4),(1,2,3,40)).save(raw,format='AVIF')
            blob=raw.getvalue();name=f'source-{m.sha(blob)}.avif';(art/name).write_bytes(blob)
            index=json.loads((art/'index.json').read_bytes());index['images'][0].update(source_file=name,source_sha256=m.sha(blob),source_bytes=len(blob),source_size=[4,4])
            self.write(art/'index.json',index)
            report,files=m.audit(*args)
            self.assertEqual(report['summary']['original_artwork_files'],1)
            self.assertEqual(report['summary']['artwork_identities_without_original'],0)
            self.assertIn('device-pack/'+name,files)
            (art/name).write_bytes(b'tampered')
            with self.assertRaises(ValueError):m.audit(*args)

if __name__=='__main__':unittest.main()
