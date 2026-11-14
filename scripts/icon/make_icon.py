# -*- coding: utf-8 -*-
"""
「LPasistant」App 图标生成器(定稿)
--------------------------------
标志:电子漂移运动曲线(回旋运动 + 漂移,摆线族)一个周期的切片 ——
  左侧圆环  = 电子的回旋圈(漂移中的转圈),
  中间自交叉 = 漂移穿过旧轨迹(剪刀轴),
  右侧开口  = 漂移继续前进的方向(刀刃)。
整体像一把横着摆的剪刀。白底黑标。

设计坐标系 0..100,y 向上;输出时翻转到屏幕坐标系。
产物:
  - ic_launcher_foreground.xml (Android 自适应图标前景层, 108dp viewport)
  - icon.svg (512 预览/源文件, 白色圆角底 + 黑标)
  - preview.png (效果预览:大图 + 48/32/16 小尺寸 + 圆形遮罩模拟)
"""
import math, os
from PIL import Image, ImageDraw, ImageFont

OUT = os.path.dirname(os.path.abspath(__file__))

# ---------------- 曲线参数(设计坐标, y-up) ----------------
RING_C   = (18.0, 50.0)   # 回旋环圆心
RING_R   = 15.2            # 回旋环半径
PIVOT    = (55.0, 50.0)   # 自交叉点(剪刀轴)
ARM2TIP  = 34.0           # 轴点到刃尖长度
BOW      = 1.2            # 刃部微微外弓
STROKE   = 8.6            # 线宽(设计单位)

def build_segments():
    """一条连续平滑曲线:上刃尖 -> 轴点 -> 下柄 -> 绕环一周 -> 上柄 -> 轴点 -> 下刃尖"""
    C, r, P = RING_C, RING_R, PIVOT
    dx, dy = P[0]-C[0], P[1]-C[1]
    d = math.hypot(dx, dy)
    ux, uy = dx/d, dy/d
    ca, sa = r/d, math.sqrt(1 - (r/d)**2)
    n_up = (ux*ca - uy*sa, ux*sa + uy*ca)          # 上切点法向
    T1 = (C[0] + r*n_up[0], C[1] + r*n_up[1])      # 上切点(环与上柄相切)
    T2 = (C[0] + r*n_up[0], C[1] - r*n_up[1])      # 下切点
    Lu = math.hypot(P[0]-T2[0], P[1]-T2[1])
    u = ((P[0]-T2[0])/Lu, (P[1]-T2[1])/Lu)         # 柄方向(下切点 -> 轴点)
    tip_ru = (P[0] + ARM2TIP*u[0], P[1] + ARM2TIP*u[1])

    def mir(p): return (p[0], 100 - p[1])
    segs = []

    # 上刃尖 -> 轴点(微微外弓)
    nrm = (-u[1], u[0])
    if nrm[1] < 0: nrm = (-nrm[0], -nrm[1])
    v = (tip_ru[0]-P[0], tip_ru[1]-P[1])
    segs.append((tip_ru,
                 (tip_ru[0]-0.30*v[0]+BOW*nrm[0], tip_ru[1]-0.30*v[1]+BOW*nrm[1]),
                 (P[0]+0.30*v[0]+BOW*nrm[0], P[1]+0.30*v[1]+BOW*nrm[1]), P))
    # 轴点 -> 下切点(直线,构成交叉的一支)
    segs.append((P, (P[0]+0.33*(T2[0]-P[0]), P[1]+0.33*(T2[1]-P[1])),
                    (T2[0]-0.33*(T2[0]-P[0]), T2[1]-0.33*(T2[1]-P[1])), T2))
    # 绕环:下切点顺时针 -> 底 -> 左 -> 顶 -> 上切点(4 段圆弧近似)
    a0 = math.atan2(-n_up[1], n_up[0])
    a1 = math.atan2(n_up[1], n_up[0])
    def pt(a): return (C[0] + r*math.cos(a), C[1] + r*math.sin(a))
    path = [a0, -math.pi/2, -math.pi, -3*math.pi/2, a1 - 2*math.pi]
    for th0, th1 in zip(path, path[1:]):
        k = 4/3 * math.tan(abs(th1-th0)/4) * r
        p0, p3 = pt(th0), pt(th1)
        segs.append((p0,
                     (p0[0] + k*math.sin(th0), p0[1] - k*math.cos(th0)),
                     (p3[0] - k*math.sin(th1), p3[1] + k*math.cos(th1)), p3))
    # 上切点 -> 轴点(直线)
    segs.append((T1, (T1[0]+0.33*(P[0]-T1[0]), T1[1]+0.33*(P[1]-T1[1])),
                     (P[0]-0.33*(P[0]-T1[0]), P[1]-0.33*(P[1]-T1[1])), P))
    # 轴点 -> 下刃尖(上刃镜像)
    s0 = segs[0]
    segs.append((P, mir(s0[2]), mir(s0[1]), mir(tip_ru)))
    return segs

def flatten(segs, n=48):
    pts = []
    for p0, p1, p2, p3 in segs:
        for i in range(n + 1):
            t = i / n; u = 1 - t
            pts.append((u**3*p0[0] + 3*u*u*t*p1[0] + 3*u*t*t*p2[0] + t**3*p3[0],
                        u**3*p0[1] + 3*u*u*t*p1[1] + 3*u*t*t*p2[1] + t**3*p3[1]))
    out = [pts[0]]
    for p in pts[1:]:
        if abs(p[0]-out[-1][0]) > 1e-9 or abs(p[1]-out[-1][1]) > 1e-9:
            out.append(p)
    return out

# ---------------- 布局:把曲线放进 108dp 视口的安全区 ----------------
def fit_viewport():
    """返回 (scale, offset)。标志含线宽全部落在中心 ⌀63 圆内(安全区 ⌀66 留边)。"""
    segs = build_segments()
    xs, ys = [], []
    for p0, p1, p2, p3 in segs:
        xs += [p0[0], p1[0], p2[0], p3[0]]
        ys += [p0[1], p1[1], p2[1], p3[1]]
    x0, x1, y0, y1 = min(xs), max(xs), min(ys), max(ys)
    cx, cy = (x0+x1)/2, (y0+y1)/2
    R_limit = 31.5 - STROKE/2 - 0.4                    # 留一点余量
    far = 0.0
    for px, py in zip(xs, ys):
        far = max(far, math.hypot(px-cx, py-cy))       # 控制点略超出曲线,足够保守
    s = R_limit / far
    ox = 54 - cx*s                                     # X = ox + x*s
    oy = 54 + cy*s                                     # Y = oy - y*s (翻转 y)
    return segs, s, ox, oy

def fmt(v):
    s = f"{v:.2f}".rstrip("0").rstrip(".")
    return s if s else "0"

def pathdata_108():
    segs, s, ox, oy = fit_viewport()
    T = lambda p: (ox + p[0]*s, oy - p[1]*s)
    p0 = T(segs[0][0])
    d = [f"M{fmt(p0[0])},{fmt(p0[1])}"]
    for a, b, c, e in segs:
        b, c, e = T(b), T(c), T(e)
        d.append(f"C{fmt(b[0])},{fmt(b[1])} {fmt(c[0])},{fmt(c[1])} {fmt(e[0])},{fmt(e[1])}")
    return " ".join(d), s

def write_android():
    d, s = pathdata_108()
    xml = f'''<?xml version="1.0" encoding="utf-8"?>
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="108dp"
    android:height="108dp"
    android:viewportWidth="108"
    android:viewportHeight="108">
    <!-- 电子漂移曲线一个周期的切片:环=回旋,交叉=漂移穿越,开口=前进方向(横摆剪刀) -->
    <path
        android:pathData="{d}"
        android:strokeColor="#000000"
        android:strokeWidth="{fmt(STROKE*s)}"
        android:strokeLineCap="round"
        android:strokeLineJoin="round"
        android:fillColor="#00000000"/>
</vector>
'''
    with open(os.path.join(OUT, "ic_launcher_foreground.xml"), "w", encoding="utf-8") as f:
        f.write(xml)

def write_svg():
    d, s = pathdata_108()
    svg = f'''<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 108 108">
  <rect x="0" y="0" width="108" height="108" rx="24" fill="#FFFFFF"/>
  <path d="{d}" fill="none" stroke="#000000" stroke-width="{fmt(STROKE*s)}" stroke-linecap="round" stroke-linejoin="round"/>
</svg>
'''
    with open(os.path.join(OUT, "icon.svg"), "w", encoding="utf-8") as f:
        f.write(svg)
    return d, s

# ---------------- Pillow 渲染预览(按 108dp 视口真实比例) ----------------
WHITE = (255, 255, 255, 255)
BLACK = (0, 0, 0, 255)

def _stamp_stroke(dr, pix, sw):
    """沿路径密集盖章圆点,得到无接缝的均匀描边"""
    rr = sw / 2.0
    step = max(0.7, sw * 0.18)
    acc = 0.0
    prev = pix[0]
    dr.ellipse([prev[0]-rr, prev[1]-rr, prev[0]+rr, prev[1]+rr], fill=BLACK)
    for p in pix[1:]:
        seg = math.hypot(p[0]-prev[0], p[1]-prev[1])
        acc += seg
        while acc >= step:
            t = 1 - (acc - step*0.5) / seg if seg > 0 else 0
            q = (prev[0] + (p[0]-prev[0])*t, prev[1] + (p[1]-prev[1])*t)
            dr.ellipse([q[0]-rr, q[1]-rr, q[0]+rr, q[1]+rr], fill=BLACK)
            acc -= step
        prev = p
    p = pix[-1]
    dr.ellipse([p[0]-rr, p[1]-rr, p[0]+rr, p[1]+rr], fill=BLACK)

def render_icon(size, ss=6, mask=None, bg=WHITE):
    """按自适应图标真实布局渲染:前景标志 fit 在中心安全圆内,背景白色圆角底。
    mask: None=不裁剪 / 'squircle'=圆角方形(常见启动器) / 'circle'=圆形"""
    segs, s, ox, oy = fit_viewport()
    pts = flatten(segs, n=64)
    W = size * ss
    k = W / 108.0
    pix = [((ox + x*s)*k, (oy - y*s)*k) for x, y in pts]
    sw = max(2, round(STROKE * s * k))
    img = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    dr = ImageDraw.Draw(img)
    dr.rounded_rectangle([0, 0, W-1, W-1], radius=int(W*0.22), fill=bg)
    _stamp_stroke(dr, pix, sw)
    img = img.resize((size, size), Image.LANCZOS)
    if mask:  # 启动器遮罩模拟
        m = Image.new("L", (size*4, size*4), 0)
        md = ImageDraw.Draw(m)
        if mask == "circle":
            md.ellipse([0, 0, size*4-1, size*4-1], fill=255)
        else:
            md.rounded_rectangle([0, 0, size*4-1, size*4-1], radius=int(size*4*0.27), fill=255)
        img.putalpha(m.resize((size, size), Image.LANCZOS))
    return img

def main():
    segs, s, ox, oy = fit_viewport()
    print(f"视口缩放 s={s:.3f},线宽={STROKE*s:.2f}dp,安全圆 ⌀{2*(31.5):.0f}")

    try:
        font = ImageFont.truetype(r"C:\Windows\Fonts\msyh.ttc", 26)
    except Exception:
        try: font = ImageFont.load_default(26)
        except TypeError: font = ImageFont.load_default()

    big = 300
    W, H = 1180, 560
    sheet = Image.new("RGB", (W, H), (246, 246, 246))
    dr = ImageDraw.Draw(sheet)
    dr.text((24, 10), "白底黑标 · 电子漂移曲线一周期切片(横摆剪刀)", fill=(0,0,0), font=font)

    dr.text((30, 55), "圆角方形遮罩(你手机的效果)", fill=(90,90,90), font=font)
    ic = render_icon(big, mask="squircle")
    sheet.paste(ic, (30, 95), ic)
    icc = render_icon(big, mask="circle")
    sheet.paste(icc, (360, 95), icc)
    dr.text((360, 410), "圆形遮罩(部分启动器)", fill=(90,90,90), font=font)
    for j, sz in enumerate([96, 48, 32, 16]):
        t = render_icon(sz, mask="squircle")
        sheet.paste(t, (700+j*115, 115), t)
    dr.text((700, 225), "96 / 48 / 32 / 16 px", fill=(90,90,90), font=font)
    dr.rectangle([690, 270, 1140, 390], fill=(32, 33, 36))
    for j, sz in enumerate([96, 48, 32]):
        t = render_icon(sz, mask="squircle")
        sheet.paste(t, (710+j*130, 280), t)
    dr.text((700, 395), "深色壁纸", fill=(90,90,90), font=font)

    sheet.save(os.path.join(OUT, "preview.png"))
    write_android()
    write_svg()
    print("已生成 preview.png / ic_launcher_foreground.xml / icon.svg")

if __name__ == "__main__":
    main()
