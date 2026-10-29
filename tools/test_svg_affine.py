import unittest
import svg_affine as a

class AffineTests(unittest.TestCase):
 def test_svg_order_and_parent_order(self):
  self.assertEqual(a.point(a.parse('translate(10 20) scale(2)'),3,4),(16,28))
  self.assertEqual(a.point(a.multiply(a.parse('scale(2)'),a.parse('translate(10 20)')),3,4),(26,48))
 def test_rotation_about_center_and_relative_cubic(self):
  self.assertEqual(a.point(a.parse('rotate(90 10 20)'),11,20),(10,21))
  self.assertEqual(a.path('m1 2h3v4c1 0 2 1 3 2z',a.parse('translate(10 20)')),'M11 22 L14 22 L14 26 C15 26 16 27 17 28 Z')
 def test_rounded_rect_quarter_turn_and_reflection(self):
  entry={'shape':{'kind':'rounded-rect','x':2,'y':3,'width':20,'height':10,'rx':4,'ry':2}}
  a.apply(entry,a.parse('translate(100 0) rotate(90)'))
  self.assertEqual(entry['shape'],{'kind':'rounded-rect','x':87,'y':2,'width':10,'height':20,'rx':2,'ry':4})
  a.apply(entry,a.parse('scale(-1 1)'))
  self.assertEqual(entry['shape']['x'],-97)
 def test_unsupported_invalid_singular_and_excessive_refuse(self):
  for value in ['skewX(20)','translate(2px)','rotate(1 2)','matrix(1 0 0 0 0 0)','scale(0)','translate(1e999)','translate(1) garbage','translate(2); scale(2)','translate(1,,2)','translate(1,)',',translate(1)','translate(1),,scale(2)']:
   with self.assertRaises(ValueError,msg=value):a.parse(value)
  with self.assertRaises(ValueError):a.apply({'shape':{'kind':'ellipse','cx':0,'cy':0,'rx':2,'ry':1}},a.parse('rotate(30)'))
  for path in ['M0 0L1','M0 0Q1 2 3 4','L1 2','M0 0C1 2 3 4 5']:
   with self.assertRaises(ValueError):a.path(path,a.parse('translate(1)'))
if __name__=='__main__':unittest.main()
