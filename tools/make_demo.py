"""Rebuild the original, self-contained textured/skinned glTF test asset. Stdlib only."""
import json, math, struct, zlib
from pathlib import Path

blob = bytearray()
views, accessors = [], []
def view(data):
    while len(blob) % 4: blob.append(0)
    i = len(views)
    views.append(dict(buffer=0, byteOffset=len(blob), byteLength=len(data)))
    blob.extend(data)
    return i
def accessor(data, kind, component=5126, bounds=False):
    count = len(data)
    flat = [v for row in data for v in row] if isinstance(data[0], (tuple, list)) else data
    fmt = {5126:'f', 5123:'H', 5125:'I'}[component]
    a = dict(bufferView=view(struct.pack('<' + fmt*len(flat), *flat)), componentType=component, count=count, type=kind)
    if bounds:
        a['min'] = [min(row[j] for row in data) for j in range(len(data[0]))]
        a['max'] = [max(row[j] for row in data) for j in range(len(data[0]))]
    accessors.append(a)
    return len(accessors)-1

def chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind+data) & 0xffffffff)
pixels = bytearray()
for y in range(64):
    pixels.append(0)
    for x in range(64):
        # Fine porcelain checker texture, visible in the shaded viewer.
        c = 255 if (x//8+y//8)%2 == 0 else 219
        pixels.extend((c,c,c,255))
png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR',struct.pack('>IIBBBBB',64,64,8,6,0,0,0)) + chunk(b'IDAT',zlib.compress(pixels)) + chunk(b'IEND',b'')
image_view = view(png)
joint_positions = [(0,.9,0),(0,1.35,0),(0,2.05,0),(-.65,1.65,0),(-.94,1.12,0),(.65,1.65,0),(.94,1.12,0),(-.29,.84,0),(-.29,.39,0),(.29,.84,0),(.29,.39,0)]
parents = [None,0,1,1,3,1,5,0,7,0,9]
names = ['Root','Torso','Head','Shoulder.L','Elbow.L','Shoulder.R','Elbow.R','Hip.L','Knee.L','Hip.R','Knee.R']
nodes=[]
for i, (p,parent,name) in enumerate(zip(joint_positions,parents,names)):
    t = p if parent is None else [p[j]-joint_positions[parent][j] for j in range(3)]
    node=dict(name=name,translation=t)
    kids=[j for j,parent_id in enumerate(parents) if parent_id == i]
    if kids: node['children']=kids
    nodes.append(node)
materials = [
    dict(name='Saffron ceramic',pbrMetallicRoughness=dict(baseColorFactor=[.95,.55,.13,1],baseColorTexture=dict(index=0),metallicFactor=.18,roughnessFactor=.36)),
    dict(name='Graphite joints',pbrMetallicRoughness=dict(baseColorFactor=[.07,.095,.12,1],metallicFactor=.65,roughnessFactor=.32)),
    dict(name='Porcelain',pbrMetallicRoughness=dict(baseColorFactor=[.84,.91,.92,1],baseColorTexture=dict(index=0),metallicFactor=.05,roughnessFactor=.4)),
    dict(name='Visor',pbrMetallicRoughness=dict(baseColorFactor=[.012,.038,.045,1],metallicFactor=.35,roughnessFactor=.23)),
    dict(name='Signal',pbrMetallicRoughness=dict(baseColorFactor=[.13,.85,.68,1],metallicFactor=.1,roughnessFactor=.3),emissiveFactor=[.08,.6,.4]),
]
parts = {}
def ellipsoid(center, scale, joint, material, rings=14, sides=24):
    p,n,uv,js,ws,idx = parts.setdefault(material, ([],[],[],[],[],[]))
    offset=len(p)
    for row in range(rings+1):
        v=row/rings
        theta=v*math.pi
        for col in range(sides+1):
            u=col/sides
            phi=u*math.tau
            unit=(math.sin(theta)*math.cos(phi),math.cos(theta),math.sin(theta)*math.sin(phi))
            p.append(tuple(center[k]+unit[k]*scale[k] for k in range(3)))
            normal=[unit[k]/scale[k] for k in range(3)]
            length=math.sqrt(sum(x*x for x in normal))
            n.append(tuple(x/length for x in normal))
            uv.append((u,v)); js.append((joint,0,0,0)); ws.append((1,0,0,0))
    for row in range(rings):
        for col in range(sides):
            a=offset+row*(sides+1)+col; b=a+sides+1
            idx.extend((a,a+1,b,a+1,b+1,b))

ellipsoid((0,1.37,0),(.55,.57,.34),1,0)
ellipsoid((0,.88,0),(.4,.22,.28),0,1)
ellipsoid((0,1.96,0),(.16,.15,.16),2,1)
ellipsoid((0,2.27,0),(.46,.36,.33),2,2)
ellipsoid((0,2.27,.29),(.36,.2,.085),2,3)
for x in [-.15,.15]: ellipsoid((x,2.28,.367),(.065,.085,.024),2,4,8,16)
ellipsoid((0,1.47,.326),(.13,.13,.028),1,4,10,20)
for side, shoulder, elbow, hip, knee in [(-1,3,4,7,8),(1,5,6,9,10)]:
    ellipsoid((side*.64,1.65,0),(.17,.17,.17),shoulder,1)
    ellipsoid((side*.82,1.39,0),(.155,.31,.16),shoulder,2)
    ellipsoid((side*.94,1.12,0),(.14,.14,.14),elbow,1)
    ellipsoid((side*.98,.91,.025),(.17,.24,.18),elbow,0)
    ellipsoid((side*.98,.69,.03),(.13,.14,.13),elbow,1)
    ellipsoid((side*.29,.66,0),(.18,.26,.18),hip,2)
    ellipsoid((side*.29,.39,0),(.14,.14,.14),knee,1)
    ellipsoid((side*.29,.23,.04),(.18,.19,.2),knee,0)
    ellipsoid((side*.29,.11,.14),(.21,.11,.31),knee,1)
primitives=[]
for material,(p,n,uv,js,ws,idx) in parts.items():
    primitives.append(dict(attributes=dict(POSITION=accessor(p,'VEC3',bounds=True),NORMAL=accessor(n,'VEC3'),TEXCOORD_0=accessor(uv,'VEC2'),JOINTS_0=accessor(js,'VEC4',5123),WEIGHTS_0=accessor(ws,'VEC4')),indices=accessor(idx,'SCALAR',5125),material=material))
inverse=[]
for x,y,z in joint_positions: inverse.append((1,0,0,0,0,1,0,0,0,0,1,0,-x,-y,-z,1))
inverse_accessor=accessor(inverse,'MAT4')
nodes.append(dict(name='Orbit bot mesh',mesh=0,skin=0))
def rotation_z(angle): return (0,0,math.sin(angle/2),math.cos(angle/2))
def rotation_y(angle): return (0,math.sin(angle/2),0,math.cos(angle/2))
def animation(name, tracks):
    samplers, channels=[],[]
    for node,path,times,values in tracks:
        samplers.append(dict(input=accessor([(t,) for t in times],'SCALAR',bounds=True),output=accessor(values,'VEC4' if path=='rotation' else 'VEC3'),interpolation='LINEAR'))
        channels.append(dict(sampler=len(samplers)-1,target=dict(node=node,path=path)))
    return dict(name=name,samplers=samplers,channels=channels)
times=[0,.5,1,1.5,2,2.5,3]
animations=[animation('Hello, world',[
    (5,'rotation',times,[rotation_z(a) for a in [0,.6,1.3,1.4,1.3,.6,0]]),
    (6,'rotation',times,[rotation_z(a) for a in [0,.4,.8,.2,.8,.4,0]]),
    (2,'rotation',times,[rotation_y(a) for a in [0,-.15,-.25,0,.25,.15,0]]),
]),animation('Idle / breathe',[
    (0,'translation',[0,1,2],[(0,.9,0),(0,.96,0),(0,.9,0)]),
    (1,'scale',[0,1,2],[(1,1,1),(1.035,1.025,1.035),(1,1,1)]),
    (2,'rotation',[0,1,2],[rotation_y(-.12),rotation_y(.12),rotation_y(-.12)]),
])]
doc=dict(asset=dict(version='2.0',generator='GLB Studio original demo generator'),scene=0,scenes=[dict(nodes=[0,len(nodes)-1])],nodes=nodes,meshes=[dict(name='Orbit bot',primitives=primitives)],skins=[dict(name='Robot rig',joints=list(range(len(joint_positions))),skeleton=0,inverseBindMatrices=inverse_accessor)],materials=materials,images=[dict(bufferView=image_view,mimeType='image/png')],textures=[dict(source=0,sampler=0)],samplers=[dict(magFilter=9729,minFilter=9729,wrapS=10497,wrapT=10497)],animations=animations,accessors=accessors,bufferViews=views,buffers=[dict(byteLength=len(blob))])
js=json.dumps(doc,separators=(',',':')).encode()
while len(js)%4: js+=b' '
while len(blob)%4: blob.append(0)
glb=struct.pack('<III',0x46546c67,2,12+8+len(js)+8+len(blob))+struct.pack('<II',len(js),0x4e4f534a)+js+struct.pack('<II',len(blob),0x004e4942)+blob
out=Path(__file__).resolve().parent.parent/'assets'/'orbit-bot.glb'
out.parent.mkdir(exist_ok=True)
out.write_bytes(glb)
print(f'{out}: {len(glb):,} bytes')
