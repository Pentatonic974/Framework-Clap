# Debug : save state de l'instance A, load dans l'instance B, comparer.
import ctypes, math
u32 = ctypes.c_uint32

d = r"C:\Users\Motociraptor\AppData\Local\Programs\Common\CLAP\PedalReverb.clap"
lib = ctypes.CDLL(d)
addr = ctypes.cast(getattr(lib, "clap_entry"), ctypes.c_void_p).value
mem = ctypes.string_at(addr, 40)
intf = [int.from_bytes(mem[i:i+8], 'little') for i in range(0, 40, 8)]
ctypes.CFUNCTYPE(ctypes.c_bool, ctypes.c_char_p)(intf[2])(d.encode())
fac = ctypes.CFUNCTYPE(ctypes.c_void_p, ctypes.c_char_p)(intf[4])(b"clap.plugin-factory")
fi = [int.from_bytes(ctypes.string_at(fac, 24)[i:i+8], 'little') for i in range(0, 24, 8)]
hostbuf = ctypes.create_string_buffer(112)
create = ctypes.CFUNCTYPE(ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_char_p)(fi[2])

def make_plugin():
    # create(factory, host, plugin_id) — 3 arguments
    p = create(None, ctypes.cast(hostbuf, ctypes.c_void_p), b"com.claps.pedal-reverb")
    pm = ctypes.string_at(p, 96)
    pi = [int.from_bytes(pm[i:i+8], 'little') for i in range(0, 96, 8)]
    ctypes.CFUNCTYPE(ctypes.c_bool, ctypes.c_void_p)(pi[2])(p)  # init
    return p, pi

def get_ext(pi, p, name):
    # pi = liste de pointeurs de la vtable clap_plugin ; index 10 = get_extension
    return ctypes.CFUNCTYPE(ctypes.c_void_p, ctypes.c_void_p, ctypes.c_char_p)(pi[10])(p, name)

class PI(ctypes.Structure):
    _fields_ = [("id", u32), ("flags", u32), ("cookie", ctypes.c_void_p),
                ("name", ctypes.c_char * 32), ("module", ctypes.c_char * 32),
                ("min", ctypes.c_double), ("max", ctypes.c_double),
                ("default_value", ctypes.c_double)]

# ostream/istream factices
WRITE = ctypes.CFUNCTYPE(ctypes.c_uint64, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_uint64)
READ = ctypes.CFUNCTYPE(ctypes.c_uint64, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_uint64)
buf = bytearray()
@WRITE
def on_write(_ctx, ptr, n):
    buf.extend(ctypes.string_at(ptr, n))
    return n

class OStream(ctypes.Structure):
    _fields_ = [("ctx", ctypes.c_void_p), ("write", WRITE)]

class IStream(ctypes.Structure):
    _fields_ = [("ctx", ctypes.c_void_p), ("read", READ)]
rpos = [0]
@READ
def on_read(_ctx, ptr, n):
    avail = len(buf) - rpos[0]
    if avail <= 0:
        return 0
    take = min(n, avail)
    ctypes.memmove(ptr, bytes(buf[rpos[0]:rpos[0]+take]), take)
    rpos[0] += take
    return take

pa, pia = make_plugin()
pe_a = get_ext(pia, pa, b"clap.params")
pvt = [int.from_bytes(ctypes.string_at(pe_a, 40)[i:i+8], 'little') for i in range(0, 40, 8)]
getv = ctypes.CFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, u32, ctypes.c_void_p)(pvt[2])

def get_all(p, vtable):
    out = {}
    for uid in (1534375328, 2608993236, 2493168076):
        v = ctypes.c_double()
        ok = getv(p, uid, ctypes.byref(v))
        out[uid] = v.value if ok else None
    return out

print("A avant flush :", get_all(pa, pvt))
# set Decay=0.75 via get_value? il faut set via process... simpler: read defaults

se_a = get_ext(pia, pa, b"clap.state")
svt = [int.from_bytes(ctypes.string_at(se_a, 16)[i:i+8], 'little') for i in range(0, 16, 8)]
os_ = OStream(None, on_write)
ok = ctypes.CFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_void_p)(svt[0])(pa, ctypes.byref(os_))
print("save ok:", ok, "bytes:", len(buf))
print(buf.decode(errors='replace')[:600])

import json as _json
env = _json.loads(bytes(buf))
env["params"]["1534375328"] = 0.9   # Decay modifié
env["params"]["2493168076"] = 0.25  # Mix modifié
buf.clear(); buf.extend(_json.dumps(env).encode())
print("enveloppe altérée : Decay=0.9, Mix=0.25")

pb, pib = make_plugin()
se_b = get_ext(pib, pb, b"clap.state")
svt_b = [int.from_bytes(ctypes.string_at(se_b, 16)[i:i+8], 'little') for i in range(0, 16, 8)]
pe_b = get_ext(pib, pb, b"clap.params")
pvt_b = [int.from_bytes(ctypes.string_at(pe_b, 40)[i:i+8], 'little') for i in range(0, 40, 8)]
getv_b = ctypes.CFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, u32, ctypes.c_void_p)(pvt_b[2])
print("B valeurs AVANT load :")
for uid in (1534375328, 2608993236, 2493168076):
    v = ctypes.c_double()
    ok = getv_b(pb, uid, ctypes.byref(v))
    print(f"  {uid} -> {v.value}" if ok else f"  {uid} -> ECHEC")

is_ = IStream(None, on_read)
ok = ctypes.CFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_void_p)(svt_b[1])(pb, ctypes.byref(is_))
print("load ok:", ok)
print("B valeurs APRES load :")
for uid in (1534375328, 2608993236, 2493168076):
    v = ctypes.c_double()
    ok = getv_b(pb, uid, ctypes.byref(v))
    print(f"  {uid} -> {v.value}" if ok else f"  {uid} -> ECHEC")
