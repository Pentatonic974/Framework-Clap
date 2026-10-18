# Simulation complete d'un host CLAP (cycle REAPER) sur le .clap installe.
# Structures ctypes fideles a clap-sys (x64 Windows).
import ctypes, math
u32 = ctypes.c_uint32

class ClapVersion(ctypes.Structure):
    _fields_ = [("major", u32), ("minor", u32), ("revision", u32)]

class AB(ctypes.Structure):  # clap_audio_buffer
    _fields_ = [("data32", ctypes.POINTER(ctypes.POINTER(ctypes.c_float))),
                ("data64", ctypes.POINTER(ctypes.POINTER(ctypes.c_double))),
                ("channel_count", u32), ("latency", u32), ("constant_mask", ctypes.c_uint64)]

class IEV(ctypes.Structure):  # clap_input_events
    _fields_ = [("ctx", ctypes.c_void_p),
                ("size", ctypes.CFUNCTYPE(u32, ctypes.c_void_p)),
                ("get", ctypes.c_void_p)]

def evsize(_):
    return 0
SIZEFN = ctypes.CFUNCTYPE(u32, ctypes.c_void_p)(evsize)

class PROC(ctypes.Structure):
    _fields_ = [("steady_time", ctypes.c_int64), ("frames_count", u32),
                ("transport", ctypes.c_void_p),
                ("audio_inputs", ctypes.POINTER(AB)),
                ("audio_outputs", ctypes.POINTER(AB)),
                ("audio_inputs_count", u32), ("audio_outputs_count", u32),
                ("in_events", ctypes.POINTER(IEV)),
                ("out_events", ctypes.POINTER(IEV))]

class PI(ctypes.Structure):  # clap_param_info
    _fields_ = [("id", u32), ("flags", u32), ("cookie", ctypes.c_void_p),
                ("name", ctypes.c_char * 32), ("module", ctypes.c_char * 32),
                ("min", ctypes.c_double), ("max", ctypes.c_double),
                ("default_value", ctypes.c_double)]

dll_path = r"C:\Users\Motociraptor\AppData\Local\Programs\Common\CLAP\PedalReverb.clap"
lib = ctypes.CDLL(dll_path)
addr = ctypes.cast(getattr(lib, "clap_entry"), ctypes.c_void_p).value
mem = ctypes.string_at(addr, 40)
intf = [int.from_bytes(mem[i:i+8], 'little') for i in range(0, 40, 8)]

init_f = ctypes.CFUNCTYPE(ctypes.c_bool, ctypes.c_char_p)(intf[2])
assert init_f(dll_path.encode()), "entry.init FAILED"
print("entry.init: OK")

fac = ctypes.CFUNCTYPE(ctypes.c_void_p, ctypes.c_char_p)(intf[4])(b"clap.plugin-factory")
assert fac, "get_factory FAILED"
fm = ctypes.string_at(fac, 24)
fi = [int.from_bytes(fm[i:i+8], 'little') for i in range(0, 24, 8)]

HOST_SIZE = 112
hostbuf = ctypes.create_string_buffer(HOST_SIZE)
# Récupère l'ID exact depuis le descripteur de la factory (ne jamais coder en dur).
GPD = ctypes.CFUNCTYPE(ctypes.c_void_p, ctypes.c_void_p, u32)(fi[1])
desc0 = GPD(None, 0)
desc_mem = ctypes.string_at(desc0, 64)
# clap_plugin_descriptor : clap_version = 3 u32 (12 octets) + pad 4, id (ptr) @16
id_ptr = int.from_bytes(desc_mem[16:24], 'little')
plugin_id = ctypes.string_at(id_ptr)
print("plugin id (descripteur) =", plugin_id.decode())
# Signature réelle : create(factory, host, plugin_id) — 3 arguments !
create_f = ctypes.CFUNCTYPE(ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_char_p)(fi[2])
plugin = create_f(None, ctypes.cast(hostbuf, ctypes.c_void_p), plugin_id)
assert plugin, "create_plugin FAILED"
print("create_plugin: OK")

pm = ctypes.string_at(plugin, 96)
pi = [int.from_bytes(pm[i:i+8], 'little') for i in range(0, 96, 8)]
names = ["desc", "self", "init", "destroy", "activate", "deactivate", "start", "stop",
         "reset", "process", "get_ext", "on_main"]
F = {n: pi[i] for i, n in enumerate(names)}

assert ctypes.CFUNCTYPE(ctypes.c_bool, ctypes.c_void_p)(F["init"])(plugin), "plugin.init FAILED"
print("plugin.init: OK")

N = 512
act = ctypes.CFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_double, u32, u32)(F["activate"])
assert act(plugin, 48000.0, 32, N), "activate FAILED"
print("activate(48000,32,512): OK")
assert ctypes.CFUNCTYPE(ctypes.c_bool, ctypes.c_void_p)(F["start"])(plugin), "start FAILED"
print("start_processing: OK")

L = (ctypes.c_float * N)()
R = (ctypes.c_float * N)()
chan_in = (ctypes.POINTER(ctypes.c_float) * 2)(ctypes.cast(L, ctypes.POINTER(ctypes.c_float)),
                                                ctypes.cast(R, ctypes.POINTER(ctypes.c_float)))
chan_out = (ctypes.POINTER(ctypes.c_float) * 2)(ctypes.cast(R, ctypes.POINTER(ctypes.c_float)),
                                                 ctypes.cast(L, ctypes.POINTER(ctypes.c_float)))
in_ab = AB(); in_ab.data32 = ctypes.cast(chan_in, ctypes.POINTER(ctypes.POINTER(ctypes.c_float))); in_ab.channel_count = 2
out_ab = AB(); out_ab.data32 = ctypes.cast(chan_out, ctypes.POINTER(ctypes.POINTER(ctypes.c_float))); out_ab.channel_count = 2
iev = IEV(ctx=None, size=SIZEFN, get=None)
oev = IEV(ctx=None, size=SIZEFN, get=None)
proc = PROC(steady_time=0, frames_count=N, transport=None,
            audio_inputs=ctypes.pointer(in_ab), audio_outputs=ctypes.pointer(out_ab),
            audio_inputs_count=1, audio_outputs_count=1,
            in_events=ctypes.pointer(iev), out_events=ctypes.pointer(oev))

process_f = ctypes.CFUNCTYPE(u32, ctypes.c_void_p, ctypes.c_void_p)(F["process"])
peak = 0.0
for k in range(20):
    for i in range(N):
        L[i] = 0.5 * math.sin(2 * math.pi * 220.0 * (k * N + i) / 48000.0)
    st = process_f(plugin, ctypes.byref(proc))
    peak = max(peak, max(abs(v) for v in R))
print(f"process x20: status={st} (3=CONTINUE) ; sortie max R={peak:.4f}")
assert peak > 0.01, "process produit du silence : BUS DU GRAPH ?"

get_ext = ctypes.CFUNCTYPE(ctypes.c_void_p, ctypes.c_void_p, ctypes.c_char_p)(F["get_ext"])
pe = get_ext(plugin, b"clap.params")
assert pe, "get_extension(clap.params) FAILED"
pe_fns = [int.from_bytes(ctypes.string_at(pe, 40)[i:i+8], 'little') for i in range(0, 40, 8)]
count = ctypes.CFUNCTYPE(u32, ctypes.c_void_p)(pe_fns[0])(plugin)
print("params count =", count)
assert count >= 3, "params manquants"
GIF = ctypes.CFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, u32, ctypes.c_void_p)
for i in range(count):
    info = PI()
    ok = GIF(pe_fns[1])(plugin, i, ctypes.byref(info))
    print(f"  param[{i}] ok={ok} id={info.id} name={info.name.decode(errors='replace')!r} "
          f"[{info.min:.2f}..{info.max:.2f}] def={info.default_value:.2f}")

assert get_ext(plugin, b"clap.state"), "clap.state FAILED"
assert get_ext(plugin, b"clap.audio-ports"), "clap.audio-ports FAILED"
print("extensions state+audio-ports: OK")

ctypes.CFUNCTYPE(None, ctypes.c_void_p)(F["deactivate"])(plugin)
ctypes.CFUNCTYPE(None, ctypes.c_void_p)(F["destroy"])(plugin)
ctypes.CFUNCTYPE(None)(intf[3])()
print("=== CYCLE CLAP COMPLET PASSE : init, create, activate, process, params, destroy, deinit ===")
