import numpy as np, onnxruntime as ort, sys, math
from PIL import Image
B=sys.argv[1]; I=sys.argv[2]
def sess(f):
    so=ort.SessionOptions(); so.log_severity_level=3; return ort.InferenceSession(f"{B}/{f}",so,providers=["CPUExecutionProvider"])
def detect(s,img,long_side,thr=0.6):
    w,h=img.size
    static = s.get_inputs()[0].shape[2]==640
    if static: nw=nh=640
    else:
        sc0=long_side/max(w,h); nw,nh=int(round(w*sc0/32)*32) or 32,int(round(h*sc0/32)*32) or 32
    sx,sy=nw/w,nh/h
    r=img.resize((nw,nh)); x=np.asarray(r)[:,:,::-1].astype(np.float32).transpose(2,0,1)[None]
    o=s.run(None,{s.get_inputs()[0].name:x}); boxes=[]
    for k,st in enumerate((8,16,32)):
        cls=o[k][0,:,0]; obj=o[3+k][0,:,0]; bb=o[6+k][0]; cols=nw//st
        for i in range(len(cls)):
            sco=math.sqrt(max(0,min(1,cls[i]))*max(0,min(1,obj[i])))
            if sco<thr: continue
            rr,cc=divmod(i,cols); cx=(cc+bb[i,0])*st; cy=(rr+bb[i,1])*st; ww=math.exp(bb[i,2])*st; hh=math.exp(bb[i,3])*st
            boxes.append((sco,cx-ww/2,cy-hh/2,cx+ww/2,cy+hh/2))
    boxes.sort(reverse=True); keep=[]
    for b in boxes:
        ok=True
        for k in keep:
            ix=max(0,min(b[3],k[3])-max(b[1],k[1])); iy=max(0,min(b[4],k[4])-max(b[2],k[2])); inter=ix*iy
            u=(b[3]-b[1])*(b[4]-b[2])+(k[3]-k[1])*(k[4]-k[2])-inter
            if inter/u>0.3: ok=False;break
        if ok: keep.append(b)
    return [(sco,x0/sx,y0/sy,x1/sx,y1/sy) for sco,x0,y0,x1,y1 in keep]
models={"2023mar fp32 (fast 640)":"face_detection_yunet_2023mar.onnx","2023mar int8":"face_detection_yunet_2023mar_int8.onnx","2026may dynamisk":"face_detection_yunet_2026may.onnx"}
for img in ("largest_selfie.jpg","demo.jpg"):
    im=Image.open(f"{I}/{img}").convert("RGB"); print(img, im.size)
    for name,f in models.items():
        s=sess(f); row=[]
        for ls in ((256,512,640) if "dynamisk" in name else (640,)):
            row.append(f"{ls}px: {len(detect(s,im,ls))} ansikt(er)")
        print("  ",name,"|"," ; ".join(row))
# embedding fp32 vs int8 på ekte utsnitt (enkel bbox-utsnitt uten 5-punktsjustering)
im=Image.open(f"{I}/demo.jpg").convert("RGB"); d=sess("face_detection_yunet_2026may.onnx"); fa=sess("face_recognition_sface_2021dec.onnx"); fi=sess("face_recognition_sface_2021dec_int8.onnx")
faces=detect(d,im,640)
emb=[]
for sco,x0,y0,x1,y1 in faces:
    cx,cy=(x0+x1)/2,(y0+y1)/2; side=max(x1-x0,y1-y0)*1.15
    c=im.crop((int(cx-side/2),int(cy-side/2),int(cx+side/2),int(cy+side/2))).resize((112,112))
    x=np.asarray(c)[:,:,::-1].astype(np.float32).transpose(2,0,1)[None]
    emb.append((fa.run(None,{fa.get_inputs()[0].name:x})[0][0], fi.run(None,{fi.get_inputs()[0].name:x})[0][0]))
def cos(a,b): return float(a@b/np.linalg.norm(a)/np.linalg.norm(b))
print("ansikter i demo.jpg:",len(emb))
print("cos(fp32,int8) samme utsnitt:",[round(cos(a,b),3) for a,b in emb])
if len(emb)>=2:
    print("fp32 par-likhet:",[round(cos(emb[i][0],emb[j][0]),2) for i in range(len(emb)) for j in range(i+1,len(emb))])
    print("int8 par-likhet:",[round(cos(emb[i][1],emb[j][1]),2) for i in range(len(emb)) for j in range(i+1,len(emb))])
