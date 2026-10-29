import base64
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec=importlib.util.spec_from_file_location('presets',Path(__file__).with_name('provision-studio-presets.py'))
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)

class PresetTests(unittest.TestCase):
    def document(self,shape=None):
        shape=shape or '<path class="selection" data-row="0" data-col="1" d="M0 0H20V10H0Z"/>'
        svg=f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50">{shape}</svg>'.encode()
        return {'productId':100,'vendorId':5426,'ledConfig':{'ledConfig_0_0':{'VID':5426,'PID':100,'EID':0,'Layout':0,'LedMatrix':[{'MatrixPos':[0,1]}]}},'svg':{'svg_0_0':base64.b64encode(svg).decode()}}
    def rows(self,document):
        return list(m.project(document,'a'*64,100))
    def test_literal_path_and_plain_rectangle_preserve_exact_identity(self):
        named=self.document();named['productName']='Razer Example';self.assertEqual(self.rows(named)[0]['product_name'],'Razer Example')
        row=self.rows(self.document())[0];data=json.loads(row['_blob']);self.assertEqual(data['keys'][0]['d'],'M0 0H20V10H0Z');self.assertEqual((data['product_id'],data['edition_id'],data['layout_id']),(100,0,0))
        row=self.rows(self.document('<rect class="selection" data-row="0" data-col="1" x="2" y="3" width="20" height="10"/>'))[0];self.assertEqual(json.loads(row['_blob'])['keys'][0]['d'],'M2.0 3.0H22.0V13.0H2.0Z')
    def test_untraced_transform_shape_missing_layout_and_matrix_refuse(self):
        for shape in ['<path class="selection" data-row="0" data-col="1" transform="skewX(5)" d="M0 0H20V10Z"/>','<circle class="selection" data-row="0" data-col="1" r="-5"/>','<rect class="selection" data-row="0" data-col="1" rx="-2" width="20" height="10"/>','<path class="selection" data-row="0" data-col="9" d="M0 0H20V10Z"/>']:
            self.assertIn('todo',self.rows(self.document(shape))[0])
        doc=self.document();doc['svg']={};self.assertIn('todo',self.rows(doc)[0])
    def test_curved_primitives_and_radius_defaults_preserve_source_geometry(self):
        circle=self.rows(self.document('<circle class="selection" data-row="0" data-col="1" cx="20" cy="10" r="5"/>'))[0]
        self.assertEqual(json.loads(circle['_blob'])['keys'][0]['shape'],{'kind':'ellipse','cx':20.,'cy':10.,'rx':5.,'ry':5.})
        ellipse=self.rows(self.document('<ellipse class="selection" data-row="0" data-col="1" cx="20" cy="10" rx="5" ry="2"/>'))[0]
        self.assertEqual(json.loads(ellipse['_blob'])['keys'][0]['shape']['ry'],2.)
        rounded=self.rows(self.document('<rect class="selection" data-row="0" data-col="1" width="20" height="10" rx="50"/>'))[0]
        self.assertEqual(json.loads(rounded['_blob'])['keys'][0]['shape'],{'kind':'rounded-rect','x':0.,'y':0.,'width':20.,'height':10.,'rx':10.,'ry':5.})
        square=self.rows(self.document('<rect class="selection" data-row="0" data-col="1" width="20" height="10" rx="0" ry="5"/>'))[0]
        self.assertIn('d',json.loads(square['_blob'])['keys'][0])
    def test_polygons_and_polylines_preserve_open_or_closed_straight_edges(self):
        for shape,end in [('polygon','Z'),('polyline','')]:
            row=self.rows(self.document(f'<{shape} class="selection" data-row="0" data-col="1" points="1,2 3,4 5,6"/>'))[0]
            self.assertEqual(json.loads(row['_blob'])['keys'][0]['d'],'M1.0 2.0L3.0 4.0L5.0 6.0'+end)
        bad=self.rows(self.document('<rect class="selection" data-row="0" data-col="1" width="20" height="10" style="rx:2"/>'))[0]
        self.assertIn('todo',bad)
    def test_nested_transform_projection_and_css_refusal(self):
        row=self.rows(self.document('<g transform="translate(10 20)"><g transform="scale(2)"><rect class="selection" data-row="0" data-col="1" x="1" y="2" width="3" height="4"/></g></g>'))[0]
        self.assertEqual(json.loads(row['_blob'])['keys'][0]['d'],'M12 24 L18 24 L18 32 L12 32 Z')
        for shape in ['<g style="transform:translate(2px)"><path class="selection" data-row="0" data-col="1" d="M0 0H10V10Z"/></g>', '<g transform="scale(0)"><circle class="selection" data-row="0" data-col="1" r="1"/></g>']:
            self.assertIn('todo',self.rows(self.document(shape))[0])
    def test_capture_hash_and_manifest_hash_are_checked_before_projection(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);blob=json.dumps(self.document()).encode();source=root/'source';source.mkdir();(source/'manifest.json').write_bytes(blob)
            record={'status':'available','product_id':100,'edition_id':0,'fetched_path':'/manifest.json','sha256':m.sha(blob),'bytes':len(blob)}
            report=root/'report.json';report.write_text(json.dumps({'stage':'lighting','records':[record]}));digest=m.sha(report.read_bytes())
            with self.assertRaises(ValueError):m.provision(report,source,root/'bad', '0'*64)
            self.assertFalse((root/'bad').exists())
            record['sha256']='0'*64;report.write_text(json.dumps({'stage':'lighting','records':[record]}));result=m.provision(report,source,root/'hash',m.sha(report.read_bytes()));self.assertEqual(result['layouts'],0);self.assertEqual(result['todos'],1)
    def test_conflicting_definitions_never_pick_an_arbitrary_winner(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);source=root/'source';source.mkdir();records=[]
            for i,shape in enumerate(['M0 0H20V10Z','M0 0H30V10Z']):
                blob=json.dumps(self.document(f'<path class="selection" data-row="0" data-col="1" d="{shape}"/>')).encode();(source/f'manifest{i}.json').write_bytes(blob);records.append({'status':'available','product_id':100,'edition_id':i,'fetched_path':f'/manifest{i}.json','sha256':m.sha(blob),'bytes':len(blob)})
            report=root/'report.json';report.write_text(json.dumps({'stage':'lighting','records':records}));result=m.provision(report,source,root/'out',m.sha(report.read_bytes()));self.assertEqual(result['layouts'],0);self.assertFalse((root/'out/studio-100-0-0.json').exists());self.assertEqual(result['todos'],1)

if __name__=='__main__':unittest.main()
