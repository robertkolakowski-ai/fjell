import sys, time, numpy as np, onnxruntime as ort, onnx
d = sys.argv[1]
def bench(f, shape, threads, seed):
    so = ort.SessionOptions(); so.intra_op_num_threads = threads
    t=time.perf_counter(); s = ort.InferenceSession(f"{d}/{f}", so, providers=["CPUExecutionProvider"]); load=(time.perf_counter()-t)*1000
    rng = np.random.default_rng(seed); x = rng.integers(0,256,size=shape).astype(np.float32)
    name = s.get_inputs()[0].name; ts=[]
    for _ in range(31):
        t=time.perf_counter(); o = s.run(None, {name:x}); ts.append((time.perf_counter()-t)*1000)
    ts.sort(); return o, load, ts[len(ts)//2], ts[int(len(ts)*0.95)]
def cos(a,b): a=a.ravel(); b=b.ravel(); return float(a@b/np.linalg.norm(a)/np.linalg.norm(b))
for f in ["face_detection_yunet_2023mar.onnx","face_detection_yunet_2023mar_int8.onnx"]:
    for th in (1,4):
        o,l,m,p=bench(f,(1,3,640,640),th,1); print(f"{f} 640 tråder={th}: last {l:.1f} ms, median {m:.1f} ms, p95 {p:.1f} ms, ut {[tuple(x.shape) for x in o][:3]}...")
    m = onnx.load(f"{d}/{f}"); print("  opset", [(i.domain or 'ai.onnx', i.version) for i in m.opset_import], "ops:", sorted({n.op_type for n in m.graph.node}))
for f in ["face_recognition_sface_2021dec.onnx","face_recognition_sface_2021dec_int8.onnx"]:
    for th in (1,4):
        o,l,m,p=bench(f,(1,3,112,112),th,7); print(f"{f} tråder={th}: last {l:.1f} ms, median {m:.1f} ms, p95 {p:.1f} ms, ut {o[0].shape}")
    m = onnx.load(f"{d}/{f}"); print("  opset", [(i.domain or 'ai.onnx', i.version) for i in m.opset_import], "ops:", sorted({n.op_type for n in m.graph.node}))
a,_,_,_=bench("face_recognition_sface_2021dec.onnx",(1,3,112,112),1,7); b,_,_,_=bench("face_recognition_sface_2021dec_int8.onnx",(1,3,112,112),1,7); c,_,_,_=bench("face_recognition_sface_2021dec_int8.onnx",(1,3,112,112),1,99)
print("cos fp32 vs int8 (samme inndata):", round(cos(a[0],b[0]),4), " cos int8 mot annen inndata:", round(cos(b[0],c[0]),4))
