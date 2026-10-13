/* AnimationCraft HTML5 Canvas runtime (MIT OR Apache-2.0).
 * Plays a movie published by AnimationCraft (AC_MOVIE): precomputed display
 * lists per frame, frame scripts with a CreateJS-style API, buttons and
 * Event/Start/Stop/Stream sounds. */
(function () {
  "use strict";
  var M = window.AC_MOVIE;
  var cv = document.getElementById("ac-stage");
  var ctx = cv.getContext("2d");
  var dpr = Math.max(1, Math.min(3, window.devicePixelRatio || 1));
  cv.width = Math.round(M.w * dpr);
  cv.height = Math.round(M.h * dpr);
  cv.style.width = M.w + "px";
  cv.style.height = M.h + "px";
  var BASE = new DOMMatrix([dpr, 0, 0, dpr, 0, 0]);
  var N = M.frames.length;
  var dirty = true;

  var paths = M.paths.map(function (s) { return new Path2D(s); });
  var images = M.images.map(function (d) {
    var im = new Image();
    im.onload = function () { dirty = true; };
    im.src = d[0];
    return { img: im, smooth: d[1] };
  });
  var clipCache = [];
  function clipPath(i) {
    if (!clipCache[i]) {
      var p = new Path2D();
      M.clips[i].forEach(function (s) { p.addPath(paths[s[0]], new DOMMatrix(s[1])); });
      clipCache[i] = p;
    }
    return clipCache[i];
  }

  // ---------------------------------------------------------------- matrices
  function decompose(m) {
    var a = m[0], b = m[1], c = m[2], d = m[3];
    var sx = Math.hypot(a, b), sy = Math.hypot(c, d);
    var det = a * d - b * c;
    var ky = Math.atan2(b, a), kx = Math.atan2(-c, d);
    if (det < 0) { sy = -sy; kx = Math.atan2(c, -d); }
    return { tx: m[4], ty: m[5], sx: sx, sy: sy, kx: kx, ky: ky };
  }
  function compose(o) {
    return [o.sx * Math.cos(o.ky), o.sx * Math.sin(o.ky), -o.sy * Math.sin(o.kx), o.sy * Math.cos(o.kx), o.tx, o.ty];
  }

  // ---------------------------------------------------------------- state
  var frame = 0, playing = true;
  var insts = {};       // name → script object (persists while playing)
  var changed = {};     // name → true once a script changed it
  var hover = null, press = null; // button src
  var scriptCache = {};

  function extra(f) { return M.extra[f] || {}; }
  function sceneOf(f) {
    for (var i = M.scenes.length - 1; i >= 0; i--) if (f >= M.scenes[i].start) return M.scenes[i];
    return M.scenes[0];
  }

  function instObj(name) {
    var o = insts[name];
    if (!o) {
      o = insts[name] = { name: name, _l: {} };
      o.addEventListener = o.on = function (type, fn) { (o._l[type] = o._l[type] || []).push(fn); return fn; };
      o.removeEventListener = o.off = function (type, fn) { o._l[type] = (o._l[type] || []).filter(function (g) { return g !== fn; }); };
      o.removeAllEventListeners = function () { o._l = {}; };
    }
    return o;
  }
  // refresh script objects from the frame (unless a script changed them)
  function bindInstances(f) {
    var n = extra(f).n || {};
    Object.keys(n).forEach(function (name) {
      var o = instObj(name), b = n[name];
      if (!changed[name]) { o.x = b.x; o.y = b.y; o.rotation = b.r; o.scaleX = b.sx; o.scaleY = b.sy; o.alpha = b.a; o.visible = true; }
      tl[name] = o;
    });
  }
  function snapshot(o) { return [o.x, o.y, o.rotation, o.scaleX, o.scaleY, o.alpha, o.visible].join(","); }

  // ---------------------------------------------------------------- drawing
  var pool = [];
  function offscreen(depth) {
    var c = pool[depth];
    if (!c) { c = pool[depth] = document.createElement("canvas"); }
    if (c.width !== cv.width || c.height !== cv.height) { c.width = cv.width; c.height = cv.height; }
    var x = c.getContext("2d");
    x.setTransform(1, 0, 0, 1, 0, 0);
    x.clearRect(0, 0, c.width, c.height);
    return c;
  }

  // per-frame: overrides by src, button substitutions
  var ov = {}, alt = {}, altDone = {};
  function prepare(f) {
    ov = {}; alt = {}; altDone = {};
    var n = extra(f).n || {};
    Object.keys(n).forEach(function (name) {
      if (!changed[name]) return;
      var o = insts[name], b = n[name];
      if (!o.visible) { ov[b.src] = { hide: true }; return; }
      var d = decompose(b.m);
      var rot = o.rotation * Math.PI / 180 - d.ky;
      d.kx += rot; d.ky += rot; d.sx = +o.scaleX; d.sy = +o.scaleY; d.tx = +o.x; d.ty = +o.y;
      var want = new DOMMatrix(compose(d));
      ov[b.src] = { delta: want.multiply(new DOMMatrix(b.m).inverse()), a: b.a > 1e-4 ? o.alpha / b.a : 1 };
    });
    (extra(f).b || []).forEach(function (bt) {
      if (press === bt.src && hover === bt.src) alt[bt.src] = bt.down;
      else if (hover === bt.src && !press) alt[bt.src] = bt.over;
    });
  }

  function drawList(c, list, depth) {
    for (var i = 0; i < list.length; i++) {
      var e = list[i];
      if (typeof e === "number") { drawItem(c, M.items[e], depth); continue; }
      var off = offscreen(depth + 1);
      drawList(off.getContext("2d"), e.c, depth + 1);
      var fx = M.fx[e.x];
      c.save();
      c.setTransform(1, 0, 0, 1, 0, 0);
      if (fx.f) c.filter = fx.f;
      c.globalCompositeOperation = fx.o;
      c.drawImage(off, 0, 0);
      c.restore();
    }
  }

  function drawItem(c, it, depth) {
    var o = it.src !== undefined ? ov[it.src] : null;
    if (it.src !== undefined && alt[it.src]) {
      if (!altDone[it.src]) { altDone[it.src] = true; var l = alt[it.src]; alt[it.src] = null; drawList(c, l, depth); alt[it.src] = l; }
      return;
    }
    if (o && o.hide) return;
    var m = new DOMMatrix(it.m);
    if (o) m = o.delta.multiply(m);
    var fa = o ? o.a : 1;
    var p = paths[it.p];
    c.save();
    if (it.c) {
      c.setTransform(BASE);
      for (var k = 0; k < it.c.length; k++) c.clip(clipPath(it.c[k]), "nonzero");
    }
    var T = BASE.multiply(m);
    c.setTransform(T);
    c.globalAlpha = fa * (it.a === undefined ? 1 : it.a);
    if (it.ol) {
      c.lineWidth = 1 / Math.max(1e-3, Math.sqrt(Math.abs(T.a * T.d - T.b * T.c)));
      c.strokeStyle = it.ol;
      c.stroke(p);
      c.restore();
      return;
    }
    if (it.img !== undefined) {
      var im = images[it.img];
      c.imageSmoothingEnabled = im.smooth;
      if (im.img.complete && im.img.naturalWidth) c.drawImage(im.img, 0, 0);
    } else if (it.pat !== undefined) {
      var pi = images[it.pat];
      if (pi.img.complete && pi.img.naturalWidth) {
        var pat = c.createPattern(pi.img, "repeat");
        if (pat.setTransform) pat.setTransform(new DOMMatrix(it.pm));
        c.imageSmoothingEnabled = pi.smooth;
        c.fillStyle = pat;
        c.fill(p, "evenodd");
      }
    } else if (it.g) {
      var gm = new DOMMatrix(it.g.m);
      var p2 = new Path2D();
      p2.addPath(p, gm.inverse());
      c.setTransform(T.multiply(gm));
      var gr = it.g.k === 0 ? c.createLinearGradient(-1, 0, 1, 0) : c.createRadialGradient(0, 0, 0, 0, 0, 1);
      it.g.s.forEach(function (s) { gr.addColorStop(Math.min(1, Math.max(0, s[0])), s[1]); });
      c.fillStyle = gr;
      c.fill(p2, "evenodd");
      c.setTransform(T);
    } else if (it.f) {
      c.fillStyle = it.f;
      c.fill(p, "evenodd");
    }
    if (it.s) {
      var cj = (it.cj || "round round").split(" ");
      c.lineCap = cj[0];
      c.lineJoin = cj[1];
      c.lineWidth = it.w;
      c.setLineDash(it.d || []);
      c.strokeStyle = it.s;
      c.stroke(p);
    }
    c.restore();
  }

  function draw() {
    prepare(frame);
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.globalAlpha = 1;
    ctx.fillStyle = M.bg;
    ctx.fillRect(0, 0, cv.width, cv.height);
    drawList(ctx, M.frames[frame], 0);
    dirty = false;
  }

  // ---------------------------------------------------------------- sound
  var actx = null, buffers = [], voices = [];
  try {
    actx = new (window.AudioContext || window.webkitAudioContext)();
    M.sounds.forEach(function (s, i) {
      var bin = atob(s[1].split(",")[1]);
      var u8 = new Uint8Array(bin.length);
      for (var k = 0; k < bin.length; k++) u8[k] = bin.charCodeAt(k);
      actx.decodeAudioData(u8.buffer, function (b) { buffers[i] = b; }, function () { console.warn("AnimationCraft: can't decode sound " + s[0]); });
    });
  } catch (e) { actx = null; }
  function unlock() { if (actx && actx.state === "suspended") actx.resume(); }
  window.addEventListener("pointerdown", unlock, true);
  window.addEventListener("keydown", unlock, true);

  function startSound(e, f) {
    if (!actx) return;
    var alive = voices.filter(function (v) { return v.alive; });
    if (e.y === "stop") { alive.forEach(function (v) { if (v.s === e.s) stopVoice(v); }); return; }
    if (e.y === "start" && alive.some(function (v) { return v.s === e.s; })) return;
    var b = buffers[e.s];
    if (!b) return;
    var src = actx.createBufferSource();
    src.buffer = b;
    src.loop = e.l || e.r > 1;
    src.connect(actx.destination);
    var t0 = actx.currentTime;
    src.start(t0);
    if (!e.l && e.r > 1) src.stop(t0 + b.duration * e.r);
    var v = { s: e.s, src: src, alive: true, stream: e.y === "stream", from: f, end: e.e };
    src.onended = function () { v.alive = false; };
    voices.push(v);
    if (voices.length > 64) voices = voices.filter(function (x) { return x.alive; });
  }
  function stopVoice(v) { if (v.alive) { try { v.src.stop(); } catch (e) { /* already stopped */ } v.alive = false; } }
  function cutStreams() {
    voices.forEach(function (v) { if (v.stream && v.alive && (!playing || frame >= v.end || frame < v.from)) stopVoice(v); });
  }

  // ---------------------------------------------------------------- scripts
  var jump = null;
  function target(t) {
    var sc = sceneOf(frame);
    if (typeof t === "number" || /^\s*-?\d+(\.\d+)?\s*$/.test(String(t))) return Math.max(0, Math.min(N - 1, sc.start + Math.floor(+t)));
    var si = M.scenes.indexOf(sc), hit = null;
    M.labels.forEach(function (l) { if (l[0] === t && (hit === null || l[1] === si)) hit = M.scenes[l[1]].start + l[2]; });
    if (hit === null) throw new Error('no frame label "' + t + '"');
    return Math.max(0, Math.min(N - 1, hit));
  }
  var tl = {
    stop: function () { playing = false; },
    play: function () { playing = true; },
    gotoAndPlay: function (f) { jump = { f: target(f), play: true }; },
    gotoAndStop: function (f) { jump = { f: target(f), play: false }; },
    nextFrame: function () { jump = { f: Math.min(N - 1, frame + 1), play: false }; },
    prevFrame: function () { jump = { f: Math.max(0, frame - 1), play: false }; },
    getChildByName: function (n) { return tl[n] || null; },
  };
  Object.defineProperty(tl, "currentFrame", { get: function () { return frame - sceneOf(frame).start; } });
  Object.defineProperty(tl, "totalFrames", { get: function () { return sceneOf(frame).count; } });

  function runScript(code) {
    var fn = scriptCache[code];
    if (!fn) {
      try {
        fn = scriptCache[code] = new Function("stop", "play", "gotoAndPlay", "gotoAndStop", "print", code);
      } catch (e) { console.error("AnimationCraft frame script:", e); scriptCache[code] = function () {}; return; }
    }
    var names = Object.keys(insts), before = {};
    names.forEach(function (n) { before[n] = snapshot(insts[n]); });
    try {
      fn.call(tl, tl.stop, tl.play, tl.gotoAndPlay, tl.gotoAndStop, function () { console.log.apply(console, arguments); });
    } catch (e) {
      console.error("AnimationCraft frame " + (frame + 1) + " script:", e);
      playing = false;
    }
    Object.keys(insts).forEach(function (n) { if (before[n] !== undefined && before[n] !== snapshot(insts[n])) changed[n] = true; });
  }

  // entering a frame: sounds, then scripts (following up to 8 jumps)
  function enter() {
    for (var hop = 0; hop < 8; hop++) {
      var x = extra(frame);
      bindInstances(frame);
      cutStreams();
      (x.snd || []).forEach(function (e) { startSound(e, frame); });
      jump = null;
      (x.js || []).forEach(runScript);
      if (!jump) break;
      var same = jump.f === frame;
      frame = jump.f;
      playing = jump.play;
      if (same) break;
    }
    cutStreams();
    dirty = true;
  }

  function advance() {
    if (frame + 1 >= N) {
      if (M.loop) frame = 0;
      else { playing = false; cutStreams(); return; }
    } else frame++;
    enter();
  }

  // ---------------------------------------------------------------- input
  function stagePoint(ev) {
    var r = cv.getBoundingClientRect();
    return { x: (ev.clientX - r.left) * (M.w / r.width), y: (ev.clientY - r.top) * (M.h / r.height) };
  }
  function itemsOf(list, src, out) {
    list.forEach(function (e) {
      if (typeof e === "number") { if (M.items[e].src === src) out.push(M.items[e]); }
      else itemsOf(e.c, src, out);
    });
    return out;
  }
  function hitTest(pt) {
    var x = extra(frame), px = pt.x * dpr, py = pt.y * dpr;
    var bs = x.b || [];
    for (var i = bs.length - 1; i >= 0; i--) {
      var b = bs[i];
      var shapes = b.hit.length ? b.hit.map(function (h) { return { p: h[0], m: h[1] }; }) : itemsOf(M.frames[frame], b.src, []);
      for (var k = 0; k < shapes.length; k++) {
        ctx.setTransform(BASE.multiply(new DOMMatrix(shapes[k].m)));
        if (ctx.isPointInPath(paths[shapes[k].p], px, py)) return { src: b.src, name: b.name, button: true };
      }
    }
    var n = x.n || {};
    var names = Object.keys(n);
    for (var j = names.length - 1; j >= 0; j--) {
      var o = insts[names[j]];
      if (!o || !Object.keys(o._l).some(function (t) { return o._l[t].length; })) continue;
      var its = itemsOf(M.frames[frame], n[names[j]].src, []);
      for (var q = 0; q < its.length; q++) {
        ctx.setTransform(BASE.multiply(new DOMMatrix(its[q].m)));
        if (ctx.isPointInPath(paths[its[q].p], px, py)) return { src: n[names[j]].src, name: names[j], button: false };
      }
    }
    return null;
  }
  function fire(name, type, pt) {
    var o = name && insts[name];
    if (!o || !o._l[type]) return;
    var ev = { type: type, target: o, currentTarget: o, stageX: pt.x, stageY: pt.y };
    o._l[type].slice().forEach(function (fn) {
      try { fn.call(tl, ev); } catch (e) { console.error("AnimationCraft " + type + " handler:", e); }
    });
    if (jump) { frame = jump.f; playing = jump.play; jump = null; enter(); }
    dirty = true;
  }
  var overName = null;
  cv.addEventListener("pointermove", function (ev) {
    var pt = stagePoint(ev), h = hitTest(pt);
    var src = h && h.button ? h.src : null;
    if (src !== hover) { hover = src; dirty = true; }
    var name = h ? h.name : null;
    if (name !== overName) { fire(overName, "mouseout", pt); fire(name, "mouseover", pt); overName = name; }
    cv.style.cursor = h ? "pointer" : "default";
  });
  cv.addEventListener("pointerleave", function () { hover = null; press = null; dirty = true; });
  cv.addEventListener("pointerdown", function (ev) {
    var h = hitTest(stagePoint(ev));
    press = h && h.button ? h.src : null;
    pressName = h ? h.name : null;
    dirty = true;
  });
  var pressName = null;
  cv.addEventListener("pointerup", function (ev) {
    var pt = stagePoint(ev), h = hitTest(pt);
    if (h && h.name === pressName) fire(h.name, "click", pt);
    press = null; pressName = null;
    dirty = true;
  });

  // ---------------------------------------------------------------- loop
  var acc = 0, last = null;
  function tick(t) {
    if (last === null) last = t;
    var dt = Math.min(0.25, (t - last) / 1000);
    last = t;
    if (playing) {
      acc += dt;
      var spf = 1 / M.fps;
      while (acc >= spf) {
        acc -= spf;
        advance();
        if (!playing) { acc = 0; break; }
      }
    } else { acc = 0; cutStreams(); }
    if (dirty || Object.keys(changed).length) draw();
    requestAnimationFrame(tick);
  }
  enter();
  draw();
  requestAnimationFrame(tick);
  // for tests and embedding pages
  window.AnimationCraft = {
    get frame() { return frame; },
    get playing() { return playing; },
    gotoAndStop: function (f) { frame = Math.max(0, Math.min(N - 1, f)); playing = false; enter(); draw(); },
    play: function () { playing = true; },
    stop: function () { playing = false; },
    timeline: tl,
  };
})();
