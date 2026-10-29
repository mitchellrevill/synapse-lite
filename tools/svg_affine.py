"""Bounded SVG attribute transforms; no CSS evaluation or vendor execution.

Normalize supported paths and axis-aligned primitives into source viewport space.
Other curved primitives refuse rotation/shear rather than approximate their shape.
"""
import math
import re
from functools import lru_cache

NUMBER = r'[-+]?(?:\d*\.\d+|\d+\.?\d*)(?:[eE][-+]?\d+)?'
IDENTITY = (1., 0., 0., 1., 0., 0.)


def multiply(left, right):
    a, b, c, d, e, f = left
    g, h, i, j, k, l = right
    result = (a*g+c*h, b*g+d*h, a*i+c*j, b*i+d*j, a*k+c*l+e, b*k+d*l+f)
    if not all(math.isfinite(v) and abs(v) <= 1_000_000 for v in result):
        raise ValueError('Excessive transform matrix')
    return result


@lru_cache(maxsize=1024)
def parse(source):
    if len(source) > 4096:
        raise ValueError('Excessive transform list')
    result, end, count = IDENTITY, 0, 0
    for match in re.finditer(r'([A-Za-z]+)\s*\(([^()]*)\)', source):
        gap = source[end:match.start()]
        if not (re.fullmatch(r'\s*',gap) if count == 0 else re.fullmatch(r'(?:\s*,\s*|\s+)',gap)):
            raise ValueError('Unsupported transform syntax')
        name, arguments = match.groups()
        numbers = list(re.finditer(NUMBER, arguments))
        position = 0
        for i, number in enumerate(numbers):
            gap = arguments[position:number.start()]
            if not (re.fullmatch(r'\s*',gap) if i == 0 else re.fullmatch(r'(?:\s*,\s*|\s+)',gap)):
                raise ValueError('Unsupported transform number syntax')
            position = number.end()
        if arguments[position:].strip():
            raise ValueError('Unsupported transform units or syntax')
        values = [float(v.group()) for v in numbers]
        if not all(math.isfinite(v) and abs(v) <= 1_000_000 for v in values):
            raise ValueError('Invalid transform number')
        if name == 'translate' and len(values) in (1, 2):
            matrix = (1, 0, 0, 1, values[0], values[1] if len(values) == 2 else 0)
        elif name == 'scale' and len(values) in (1, 2):
            matrix = (values[0], 0, 0, values[-1], 0, 0)
        elif name == 'matrix' and len(values) == 6:
            matrix = tuple(values)
        elif name == 'rotate' and len(values) in (1, 3):
            angle = math.radians(values[0])
            c, s = math.cos(angle), math.sin(angle)
            # Exact quarter turns retain analytic ellipse/rounded-rect geometry.
            if values[0] % 90 == 0:
                c, s = round(c), round(s)
            matrix = (c, s, -s, c, 0, 0)
            if len(values) == 3:
                x, y = values[1:]
                matrix = multiply(multiply((1,0,0,1,x,y), matrix), (1,0,0,1,-x,-y))
        else:
            raise ValueError('Unsupported transform operation or arity')
        result = multiply(result, matrix)
        end, count = match.end(), count + 1
        if count > 64:
            raise ValueError('Excessive transform operations')
    if source[end:].strip() or (source.strip() and not count):
        raise ValueError('Unsupported transform syntax')
    if abs(result[0]*result[3]-result[1]*result[2]) < 1e-12:
        raise ValueError('Singular transform')
    return result


def point(matrix, x, y):
    a,b,c,d,e,f = matrix
    result = (a*x+c*y+e, b*x+d*y+f)
    if not all(math.isfinite(v) and abs(v) <= 1_000_000 for v in result):
        raise ValueError('Transformed coordinate exceeds budget')
    return result


def path(source, matrix):
    if len(source) > 16384 or re.sub(NUMBER + r'|[MmLlHhVvCcZz]', '', source).strip(' ,\t\n\r'):
        raise ValueError('Untraced or excessive SVG path command')
    tokens = re.findall(NUMBER + r'|[MmLlHhVvCcZz]', source)
    index, command, current, start, output = 0, None, (0.,0.), (0.,0.), []
    while index < len(tokens):
        if tokens[index].isalpha():
            command = tokens[index]
            index += 1
        if command is None:
            raise ValueError('Path requires command')
        upper, relative = command.upper(), command.islower()
        if upper == 'Z':
            output.append('Z'); current = start; command = None
            continue
        count = {'M':2, 'L':2, 'H':1, 'V':1, 'C':6}[upper]
        part = tokens[index:index+count]
        if len(part) != count or any(v.isalpha() for v in part):
            raise ValueError('Incomplete path coordinates')
        values = list(map(float, part)); index += count
        if upper == 'H':
            coords = [(values[0] + (current[0] if relative else 0), current[1])]
        elif upper == 'V':
            coords = [(current[0], values[0] + (current[1] if relative else 0))]
        else:
            coords = [(values[i]+(current[0] if relative else 0), values[i+1]+(current[1] if relative else 0)) for i in range(0,count,2)]
        if not output and upper != 'M':
            raise ValueError('Path must start with move')
        output.append(('L' if upper in ('H','V') else upper) + ' '.join(format(v,'.15g') for p in coords for v in point(matrix,*p)))
        current = coords[-1]
        if upper == 'M':
            start = current; command = 'l' if relative else 'L'
    result = ' '.join(output)
    if len(result) > 16384:
        raise ValueError('Transformed path exceeds budget')
    return result


def apply(entry, matrix):
    if matrix == IDENTITY:
        return
    if 'd' in entry:
        entry['d'] = path(entry['d'], matrix)
        return
    a,b,c,d,_,_ = matrix
    if not ((b == c == 0) or (a == d == 0)):
        raise ValueError('Non-axis-aligned curved primitive transform')
    shape = entry['shape']
    rx, ry = abs(a)*shape['rx']+abs(c)*shape['ry'], abs(b)*shape['rx']+abs(d)*shape['ry']
    if shape['kind'] == 'ellipse':
        shape['cx'], shape['cy'] = point(matrix, shape['cx'], shape['cy'])
    else:
        x,y,w,h = (shape[k] for k in ('x','y','width','height'))
        corners = [point(matrix,x,y),point(matrix,x+w,y+h)]
        shape['x'], shape['y'] = min(p[0] for p in corners), min(p[1] for p in corners)
        shape['width'], shape['height'] = abs(a)*w+abs(c)*h, abs(b)*w+abs(d)*h
    shape['rx'], shape['ry'] = rx, ry
