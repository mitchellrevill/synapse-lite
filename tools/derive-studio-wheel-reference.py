"""Independent scalar translation of recovered Wheel instructions, no DLL calls."""
import math,struct,json,hashlib
from pathlib import Path
root=Path(__file__).resolve().parent.parent
f32=lambda n:struct.unpack('<f',struct.pack('<f',n))[0]
palette=[0]*1024
value=0.;delta=f32(255./1023)
for i in range(1,1023):
 value=f32(value+delta);palette[i]=int(value)
palette[-1]=255
def frame(rows,cols,origin,center,speed,ccw,n):
 period=int(f32(f32(360000.)/f32(speed)))//40 if speed else 0
 offset=f32(f32(f32(n%period)*1024.)/f32(period)) if period else 0
 rotation=int(offset if ccw else f32(1024.-offset)) if period else 0
 return [palette[(int(1024.*(math.atan2(f32(row+origin[0]-center[0]),col+origin[1]-center[1])+math.pi)/(2*math.pi))+rotation)%1024] for row in range(rows) for col in range(cols)]
cases=[]
for speed in [0,1,60,360]:
 for ccw in [False,True]:
  origin=[121,113];center=[123,125]
  cases.append(dict(speed=speed,ccw=ccw,origin=origin,center=center,frames=[frame(3,3,origin,center,speed,ccw,n) for n in range(200)]))
profile=json.loads((root/'crates/secbird-core/tests/fixtures/studio-preview-profile.json').read_text())
leds=profile['layers'][0]['regions'][0]['devices'][0]['leds']
base={(p['row'],p['col']) for p in leds};top=set(sorted(base)[::3])
listed={tuple(p['MatrixPos']) for p in profile['devices'][0]['config']['LedMatrix']}
digest=hashlib.md5()
for n in range(200):
 values=frame(7,23,[121,113],[123,125],60,False,n)
 for row in range(7):
  for col in range(23):
   c=(row,col);gray=values[row*23+col]
   pixel=[gray]*3 if c in top else [4,16,240] if c in base else [0]*3
   digest.update(bytes(pixel if c in listed else [0]*3))
data={'authority':'independent scalar reconstruction, not vendor execution','dll_sha256':'cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa','init':'0x180066b40','frame':'0x180066d60','cases':cases,'composition_rgb_md5':digest.hexdigest()}
(root/'crates/secbird-core/tests/fixtures/studio-wheel-reference.json').write_text(json.dumps(data,separators=(',',':'))+'\n')
