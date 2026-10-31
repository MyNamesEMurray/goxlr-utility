const S = { status: null, serial: null, view: 'mixer', ws: null, online: false };
let nextId = 1, bound = [], micTimer = null;
const pending = new Map();

const h = (tag, props = {}, ...kids) => {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(props || {})) {
    if (k === 'class') e.className = v;
    else if (k.startsWith('on')) e.addEventListener(k.slice(2), v);
    else if (v !== undefined && v !== null && v !== false) e.setAttribute(k, v === true ? '' : v);
  }
  e.append(...kids.flat().filter((x) => x !== null && x !== undefined && x !== false));
  return e;
};
const $ = (s) => document.querySelector(s);
const mixer = () => S.status?.mixers?.[S.serial];
const bind = (el, get, apply) => { bound.push({ el, f: () => { const m = mixer(); if (m) { try { apply(el, get(m)); } catch { /* path not present */ } } } }); return el; };
const refresh = () => {
  for (let i = 0; i < bound.length; i++) if (bound[i].el.isConnected) bound[i].f();
  bound = bound.filter((b) => b.el.isConnected);
};

function toast(msg, ok) {
  const t = h('div', { class: 'toast' + (ok ? ' ok' : '') }, msg);
  $('#toasts').append(t);
  setTimeout(() => t.remove(), 4500);
}

function send(data) {
  return new Promise((resolve) => {
    if (!S.online) { toast('Not connected to daemon'); return resolve(null); }
    const id = nextId++;
    pending.set(id, resolve);
    S.ws.send(JSON.stringify({ id, data }));
  }).then((r) => {
    if (r && r.Error) toast(r.Error);
    return r;
  });
}
const shape = (name, args) => ({ [name]: args.length === 0 ? [] : args.length === 1 ? args[0] : args });
const cmd = (name, ...args) => send({ Command: [S.serial, shape(name, args)] });
const daemon = (name, ...args) => send({ Daemon: args.length || name.startsWith('Set') ? shape(name, args) : name });

const patchPtr = (doc, ptr) => {
  const parts = ptr.split('/').slice(1).map((p) => p.replace(/~1/g, '/').replace(/~0/g, '~'));
  const last = parts.pop();
  let o = doc;
  for (const p of parts) o = o[p];
  return [o, last];
};
const getPtr = (doc, ptr) => { const [o, k] = patchPtr(doc, ptr); return ptr === '' ? doc : o[k]; };
function applyPatch(doc, ops) {
  for (const op of ops) {
    if (op.path === '') { doc = op.value; continue; }
    const [o, k] = patchPtr(doc, op.path);
    const put = (obj, key, val) => (Array.isArray(obj) ? (key === '-' ? obj.push(val) : obj.splice(+key, 0, val)) : (obj[key] = val));
    const del = (obj, key) => (Array.isArray(obj) ? obj.splice(+key, 1) : delete obj[key]);
    if (op.op === 'add') put(o, k, op.value);
    else if (op.op === 'replace') o[k] = op.value;
    else if (op.op === 'remove') del(o, k);
    else if (op.op === 'move') { const v = getPtr(doc, op.from); const [fo, fk] = patchPtr(doc, op.from); del(fo, fk); put(o, k, v); }
    else if (op.op === 'copy') put(o, k, structuredClone(getPtr(doc, op.from)));
  }
  return doc;
}

function connect() {
  const ws = new WebSocket(`${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/api/websocket`);
  S.ws = ws;
  ws.onopen = async () => {
    S.online = true;
    const r = await send('GetStatus');
    if (r?.Status) setStatus(r.Status, true);
  };
  ws.onmessage = (ev) => {
    const msg = JSON.parse(ev.data);
    if (msg.data?.Patch) {
      if (!S.status) return;
      const before = Object.keys(S.status.mixers).join();
      try { S.status = applyPatch(S.status, msg.data.Patch); } catch (e) { console.warn('patch failed, resyncing', e); send('GetStatus').then((r) => r?.Status && setStatus(r.Status, true)); return; }
      Object.keys(S.status.mixers).join() !== before ? setStatus(S.status, true) : (refresh(), renderTop());
    } else if (pending.has(msg.id)) {
      pending.get(msg.id)(msg.data);
      pending.delete(msg.id);
    }
  };
  ws.onclose = () => { S.online = false; pending.forEach((r) => r(null)); pending.clear(); renderTop(); setTimeout(connect, 2000); };
}

function setStatus(st, rerender) {
  S.status = st;
  const serials = Object.keys(st.mixers);
  if (!serials.includes(S.serial)) S.serial = serials[0] || null;
  if (rerender) render();
}

const throttle = (fn, ms = 40) => {
  let last = 0, t = null, arg;
  return (v) => {
    arg = v;
    const now = Date.now();
    clearTimeout(t);
    if (now - last >= ms) { last = now; fn(v); } else t = setTimeout(() => { last = Date.now(); fn(arg); }, ms);
  };
};

const btn = (text, onclick, cls = '') => h('button', { class: 'b ' + cls, onclick }, text);
const row = (label, ...ctl) => h('div', { class: 'row' }, label ? h('label', {}, label) : null, ...ctl);
const card = (title, ...kids) => h('div', { class: 'card' }, h('h3', {}, title), ...kids);

const fill = (el) => {
  const span = el.max - el.min, p = ((el.value - el.min) / span) * 100, c = +el.min < 0 && +el.max > 0 ? (-el.min / span) * 100 : 0;
  el.style.setProperty('--a', Math.min(c, p) + '%');
  el.style.setProperty('--b', Math.max(c, p) + '%');
};
function rangeInput(props, get, set, onVal) {
  const input = h('input', { type: 'range', ...props });
  const send1 = throttle(set);
  input.addEventListener('input', () => { fill(input); onVal?.(+input.value); send1(+input.value); });
  return bind(input, get, (el, v) => { if (!el.matches(':active')) { el.value = v; fill(el); onVal?.(+v); } });
}
function slider(label, { min, max, step = 1, get, set, fmt = (v) => v }) {
  const val = h('span', { class: 'val' });
  return row(label, rangeInput({ min, max, step }, get, set, (v) => (val.textContent = fmt(v))), val);
}
function sliderV(get, set, { min, max, step = 1 }) {
  return rangeInput({ min, max, step, class: 'v' }, get, set);
}
function seg(label, options, get, set, { cls = '' } = {}) {
  const wrap = h('div', { class: 'seg' });
  const btns = options.map((o) => { const [v, t] = Array.isArray(o) ? o : [o, o]; const b = btn(t, () => set(v), cls); b.dataset.v = v; wrap.append(b); return b; });
  bind(wrap, get, (_, v) => btns.forEach((b) => b.classList.toggle('on', b.dataset.v === String(v))));
  return label ? row(label, wrap) : wrap;
}
function sel(label, options, get, set) {
  const opts = options.map((o) => (Array.isArray(o) ? o : [o, o]));
  const s = h('select', {}, !opts.some(([v]) => v === '') ? h('option', { value: '', disabled: true, hidden: true }, 'Select...') : null, opts.map(([v, t]) => h('option', { value: v }, t)));
  s.addEventListener('change', () => set(s.value));
  bind(s, get, (el, v) => { if (document.activeElement !== el) el.value = v ?? ''; });
  return label ? row(label, s) : s;
}
function toggle(label, get, set, cls = '') {
  if (!cls) {
    const i = h('input', { type: 'checkbox' });
    i.addEventListener('change', () => set(i.checked));
    bind(i, get, (el, v) => (el.checked = !!v));
    return h('label', { class: 'sw' }, i, h('span', { class: 'tr' }), label);
  }
  const b = btn(label, () => set(!b.classList.contains('on')), cls);
  return bind(b, get, (el, v) => el.classList.toggle('on', !!v));
}
function dyn(getter, build) {
  const box = h('div');
  let key = null;
  bound.push({ el: box, f: () => {
    const m = mixer();
    if (!m && !S.status) return;
    let v;
    try { v = getter(S.status, m); } catch { v = null; }
    const k = JSON.stringify(v);
    if (k === key) return;
    key = k;
    box.replaceChildren(...[build(v)].flat().filter(Boolean));
  } });
  return box;
}
const numInput = (get, set, props = {}) => {
  const i = h('input', { type: 'number', ...props });
  i.addEventListener('change', () => set(+i.value));
  return bind(i, get, (el, v) => { if (document.activeElement !== el) el.value = v; });
};
const prompter = (msg, fn) => () => { const v = prompt(msg); if (v && v.trim()) fn(v.trim()); };

const CHANNELS = ['Mic', 'LineIn', 'Console', 'System', 'Game', 'Chat', 'Sample', 'Music', 'Headphones', 'MicMonitor', 'LineOut'];
const CH_LABEL = { LineIn: 'Line In', MicMonitor: 'Mic Monitor', LineOut: 'Line Out', Sample: 'Samples' };
const label = (s) => CH_LABEL[s] || s.replace(/([a-z])([A-Z0-9])/g, '$1 $2');
const lopts = (arr) => arr.map((v) => [v, label(v)]);
const SHORT = { Headphones: 'Phones', BroadcastMix: 'Bcast', ChatMic: 'Chat', LineOut: 'Line', StreamMix2: 'Strm 2' };
const FADERS = ['A', 'B', 'C', 'D'];
const MUTE_FN = ['All', 'ToStream', 'ToVoiceChat', 'ToPhones', 'ToLineOut', 'ToStream2', 'ToStreams'];
const pct = (v) => Math.round((v / 255) * 100) + '%';
const isMini = () => mixer()?.hardware.device_type === 'Mini';

const views = {
  mixer() {
    const m = mixer();
    const strips = h('div', { class: 'strips' });
    for (const ch of CHANNELS) {
      const faderOf = () => FADERS.find((f) => mixer().fader_status[f].channel === ch);
      const volume = h('div', { class: 'val' });
      const tag = bind(h('div', { class: 'tag' }), () => 0, (el) => { el.textContent = faderOf() ? 'Fader ' + faderOf() : 'No fader'; el.classList.toggle('none', !faderOf()); });
      const slide = sliderV((m) => m.levels.volumes[ch], (v) => cmd('SetVolume', ch, v), { min: 0, max: 255 });
      slide.addEventListener('input', () => (volume.textContent = pct(+slide.value)));
      bind(volume, (m) => m.levels.volumes[ch], (el, v) => (el.textContent = pct(v)));
      const mute = btn('Mute', (e) => {
        const f = faderOf();
        if (!f) return;
        const cur = mixer().fader_status[f].mute_state;
        cmd('SetFaderMuteState', f, cur === 'Unmuted' ? (e.shiftKey ? 'MutedToAll' : 'MutedToX') : 'Unmuted');
      }, 'red');
      bind(mute, () => 0, (el) => { const f = faderOf(); el.disabled = !f; el.title = f ? '' : 'Not assigned to a fader'; el.classList.toggle('on', !!f && mixer().fader_status[f].mute_state !== 'Unmuted'); });
      const s = h('div', { class: 'strip' }, h('div', { class: 'name' }, label(ch)), tag, slide, volume, mute);
      bind(s, () => 0, (el) => el.classList.toggle('fader', !!faderOf()));
      strips.append(s);
    }
    const meter = h('i');
    const quick = h('div', { class: 'grid' },
      card('Microphone',
        row('Level', h('div', { class: 'vu' }, meter)),
        toggle('Cough', (m) => m.cough_button.state !== 'Unmuted', () => cmd('SetCoughMuteState', mixer().cough_button.state === 'Unmuted' ? 'MutedToX' : 'Unmuted'), 'red'),
        slider('Bleep volume', { min: -34, max: 0, get: (m) => m.levels.bleep, set: (v) => cmd('SetSwearButtonVolume', v), fmt: (v) => v + ' dB' })),
      card('Faders', ...FADERS.map((f) => h('div', { class: 'row pair' },
        h('label', { style: 'min-width:0' }, f),
        sel(null, lopts(CHANNELS.slice(0, 8)), (m) => m.fader_status[f].channel, (v) => cmd('SetFader', f, v)),
        sel(null, MUTE_FN.map((v) => [v, label(v)]), (m) => m.fader_status[f].mute_type, (v) => cmd('SetFaderMuteFunction', f, v)))),
        toggle('Lock faders', (m) => m.settings.lock_faders, (v) => cmd('SetLockFaders', v))),
      h('div', { class: 'card', style: 'grid-column:1/-1' }, h('h3', {}, 'Scribbles'), ...FADERS.map((f) => h('div', { class: 'row scr' }, h('label', { style: 'min-width:0' }, f),
        h('img', { src: `/files/scribble/${S.serial}/${f}.png?${Date.now() % 1e6}`, width: 64, style: 'image-rendering:pixelated;background:#000', onerror: (e) => (e.target.style.display = 'none') }),
        dyn((st) => st.files.icons, (icons) => sel(null, [['', '(no icon)'], ...icons.map((i) => [i, i])], (m) => m.fader_status[f].scribble?.file_name ?? '', (v) => cmd('SetScribbleIcon', f, v || null))),
        txtInput('Top', (m) => m.fader_status[f].scribble?.left_text ?? '', (v) => cmd('SetScribbleNumber', f, v)),
        txtInput('Bottom', (m) => m.fader_status[f].scribble?.bottom_text ?? '', (v) => cmd('SetScribbleText', f, v)),
        toggle('Invert', (m) => m.fader_status[f].scribble?.inverted, (v) => cmd('SetScribbleInvert', f, v))))));
    pollMic((lvl) => (meter.style.width = Math.min(100, Math.sqrt(lvl) * 100) + '%'));
    return h('div', {}, strips, quick);
  },

  routing() {
    const m = mixer();
    const inputs = Object.keys(m.router);
    const outputs = Object.keys(m.router[inputs[0]]);
    const tbl = h('div', { class: 'matrix-wrap' }, h('table', { class: 'matrix' }, h('tr', {}, h('th'), ...outputs.map((o) => h('th', { title: label(o) }, h('span', { class: 'f' }, label(o)), h('span', { class: 's' }, SHORT[o] || label(o))))),
      ...inputs.map((i) => h('tr', {}, h('th', {}, label(i)), ...outputs.map((o) => {
        const b = h('button', { title: i === 'Chat' && o === 'ChatMic' ? 'Not routable' : `${label(i)} to ${label(o)}`, 'aria-label': `${label(i)} to ${label(o)}`, disabled: i === 'Chat' && o === 'ChatMic', 'aria-disabled': i === 'Chat' && o === 'ChatMic' ? 'true' : null, onclick: () => cmd('SetRouter', i, o, !b.classList.contains('on')) });
        bind(b, (m) => m.router[i][o], (el, v) => el.classList.toggle('on', !!v));
        return h('td', {}, b);
      })))));
    const cards = [card('Routing matrix', tbl),
      card('Monitor', sel('Monitor mix', lopts(outputs), (m) => m.levels.output_monitor, (v) => cmd('SetMonitorMix', v)),
        toggle('Monitor with FX', (m) => m.settings.enable_monitor_with_fx, (v) => cmd('SetMonitorWithFx', v)))];
    if (m.levels.submix_supported) {
      const subs = ['Mic', 'LineIn', 'Console', 'System', 'Game', 'Chat', 'Sample', 'Music'];
      cards.push(card('Sub mix', toggle('Enable sub mix', (m) => !!m.levels.submix, (v) => cmd('SetSubMixEnabled', v)),
        dyn((st, m) => !!m.levels.submix, (on) => on ? [
          ...subs.map((c) => { const val = h('span', { class: 'val' }); return h('div', { class: 'row nw' }, h('label', {}, label(c)),
            rangeInput({ min: 0, max: 255 }, (m) => m.levels.submix.inputs[c].volume, (v) => cmd('SetSubMixVolume', c, v), (v) => (val.textContent = pct(v))), val,
            toggle('Link', (m) => m.levels.submix.inputs[c].linked, (v) => cmd('SetSubMixLinked', c, v))); }),
        ] : h('p', { class: 'muted' }, 'Disabled.'))),
        card('Output mix', h('p', { class: 'hint' }, 'Choose which sub mix feeds each output.'), dyn((st, m) => !!m.levels.submix, (on) => on ? outputs.map((o) => seg(label(o), ['A', 'B'], (m) => m.levels.submix.outputs[o], (v) => cmd('SetSubMixOutputMix', o, v))) : h('p', { class: 'muted' }, 'Enable sub mix first.'))));
    }
    return h('div', {}, cards[0], h('div', { class: 'masonry', style: 'margin-top:16px' }, ...cards.slice(1)));
  },

  mic() {
    const m = mixer();
    const types = ['Dynamic', 'Condenser', 'Jack'];
    const gains = (t) => (m2) => m2.mic_status.mic_gains[t];
    const GATE = ['Gate10ms', 'Gate20ms', 'Gate30ms', 'Gate40ms', 'Gate50ms', 'Gate60ms', 'Gate70ms', 'Gate80ms', 'Gate90ms', 'Gate100ms', 'Gate110ms', 'Gate120ms', 'Gate130ms', 'Gate140ms', 'Gate150ms', 'Gate160ms', 'Gate170ms', 'Gate180ms', 'Gate190ms', 'Gate200ms', 'Gate250ms', 'Gate300ms', 'Gate350ms', 'Gate400ms', 'Gate450ms', 'Gate500ms', 'Gate550ms', 'Gate600ms', 'Gate650ms', 'Gate700ms', 'Gate750ms', 'Gate800ms', 'Gate850ms', 'Gate900ms', 'Gate950ms', 'Gate1000ms', 'Gate1100ms', 'Gate1200ms', 'Gate1300ms', 'Gate1400ms', 'Gate1500ms', 'Gate1600ms', 'Gate1700ms', 'Gate1800ms', 'Gate1900ms', 'Gate2000ms'];
    const CRATIO = ['Ratio1_0', 'Ratio1_1', 'Ratio1_2', 'Ratio1_4', 'Ratio1_6', 'Ratio1_8', 'Ratio2_0', 'Ratio2_5', 'Ratio3_2', 'Ratio4_0', 'Ratio5_6', 'Ratio8_0', 'Ratio16_0', 'Ratio32_0', 'Ratio64_0'];
    const CATK = ['Comp0ms', 'Comp2ms', 'Comp3ms', 'Comp4ms', 'Comp5ms', 'Comp6ms', 'Comp7ms', 'Comp8ms', 'Comp9ms', 'Comp10ms', 'Comp12ms', 'Comp14ms', 'Comp16ms', 'Comp18ms', 'Comp20ms', 'Comp23ms', 'Comp26ms', 'Comp30ms', 'Comp35ms', 'Comp40ms'];
    const CREL = ['Comp0ms', 'Comp15ms', 'Comp25ms', 'Comp35ms', 'Comp45ms', 'Comp55ms', 'Comp65ms', 'Comp75ms', 'Comp85ms', 'Comp100ms', 'Comp115ms', 'Comp140ms', 'Comp170ms', 'Comp230ms', 'Comp340ms', 'Comp680ms', 'Comp1000ms', 'Comp1500ms', 'Comp2000ms', 'Comp3000ms'];
    const nice = (s) => [s, s.replace(/^(Gate|Comp)/, '').replace(/^Ratio/, '').replace('_', '.')];
    const bands = isMini()
      ? { prop: 'equaliser_mini', names: ['Equalizer90Hz', 'Equalizer250Hz', 'Equalizer500Hz', 'Equalizer1KHz', 'Equalizer3KHz', 'Equalizer8KHz'] }
      : { prop: 'equaliser', names: ['Equalizer31Hz', 'Equalizer63Hz', 'Equalizer125Hz', 'Equalizer250Hz', 'Equalizer500Hz', 'Equalizer1KHz', 'Equalizer2KHz', 'Equalizer4KHz', 'Equalizer8KHz', 'Equalizer16KHz'] };
    const eq = h('div', { class: 'eq' }, bands.names.map((n) => h('div', { class: 'band' },
      (() => { const g = h('span', { class: 'gv' }); return [g, rangeInput({ min: -9, max: 9, class: 'v' }, (m) => m.mic_status[bands.prop].gain[n], (v) => cmd(isMini() ? 'SetEqMiniGain' : 'SetEqGain', n, v), (v) => (g.textContent = (v > 0 ? '+' : '') + v))]; })(),
      numInput((m) => Math.round(m.mic_status[bands.prop].frequency[n]), (v) => cmd(isMini() ? 'SetEqMiniFreq' : 'SetEqFreq', n, v), { step: 1 }),
      h('span', {}, n.replace('Equalizer', '').replace('KHz', 'kHz')))));
    return h('div', {}, h('div', { class: 'masonry' },
      card('Input', seg('Type', types, (m) => m.mic_status.mic_type, (v) => cmd('SetMicrophoneType', v)),
        dyn((st, m) => m.mic_status.mic_type, (t) => slider('Gain', { min: 0, max: 72, get: gains(t), set: (v) => cmd('SetMicrophoneGain', t, v), fmt: (v) => v + ' dB' })),
        slider('De-esser', { min: 0, max: 100, get: (m) => m.levels.deess, set: (v) => cmd('SetDeeser', v), fmt: (v) => v + '%' })),
      card('Noise gate', toggle('Enabled', (m) => m.mic_status.noise_gate.enabled, (v) => cmd('SetGateActive', v)),
        slider('Threshold', { min: -59, max: 0, get: (m) => m.mic_status.noise_gate.threshold, set: (v) => cmd('SetGateThreshold', v), fmt: (v) => v + ' dB' }),
        slider('Attenuation', { min: 0, max: 100, get: (m) => m.mic_status.noise_gate.attenuation, set: (v) => cmd('SetGateAttenuation', v), fmt: (v) => v + '%' }),
        sel('Attack', GATE.map(nice), (m) => m.mic_status.noise_gate.attack, (v) => cmd('SetGateAttack', v)),
        sel('Release', GATE.map(nice), (m) => m.mic_status.noise_gate.release, (v) => cmd('SetGateRelease', v))),
      card('Compressor',
        slider('Threshold', { min: -40, max: 0, get: (m) => m.mic_status.compressor.threshold, set: (v) => cmd('SetCompressorThreshold', v), fmt: (v) => v + ' dB' }),
        slider('Makeup gain', { min: 0, max: 24, get: (m) => m.mic_status.compressor.makeup_gain, set: (v) => cmd('SetCompressorMakeupGain', v), fmt: (v) => v + ' dB' }),
        sel('Ratio', CRATIO.map(nice), (m) => m.mic_status.compressor.ratio, (v) => cmd('SetCompressorRatio', v)),
        sel('Attack', CATK.map(nice), (m) => m.mic_status.compressor.attack, (v) => cmd('SetCompressorAttack', v)),
        sel('Release', CREL.map(nice), (m) => m.mic_status.compressor.release, (v) => cmd('SetCompressorReleaseTime', v))),
      ), h('div', { class: 'card', style: 'margin-top:16px' }, h('h3', {}, 'Equaliser'), h('p', { class: 'hint' }, 'Gain in dB (-9 to +9). Centre frequency in Hz underneath.'), eq));
  },

  effects() {
    const m = mixer();
    if (!m.effects) return h('div', { class: 'empty' }, 'Effects are not available on this device.');
    const E = (path) => (m) => path.split('.').reduce((o, k) => o[k], m.effects.current);
    const fx = (title, enable, ...kids) => card(title, enable, ...kids);
    const S1 = (label_, path, c, min, max, unit = '', extra = []) => slider(label_, { min, max, get: E(path), set: (v) => cmd(c, ...extra, v), fmt: (v) => v + unit });
    const st = (path, c, opts) => sel('Style', opts.map((o) => [o, label(o)]), E(path), (v) => cmd(c, v));
    const presets = ['Preset1', 'Preset2', 'Preset3', 'Preset4', 'Preset5', 'Preset6'];
    return h('div', { class: 'cols' }, h('div', { class: 'col' },
      card('Presets', toggle('Effects on', (m) => m.effects.is_enabled, (v) => cmd('SetFXEnabled', v)),
        h('div', { class: 'row' }, seg(null, presets.map((p, i) => [p, String(i + 1)]), (m) => m.effects.active_preset, (v) => cmd('SetActiveEffectPreset', v))),
        dyn((s, m) => [m.effects.active_preset, m.effects.preset_names], () => h('p', { class: 'muted' }, 'Active preset: ' + (mixer().effects.preset_names[mixer().effects.active_preset] || ''))),
        h('div', { class: 'row' }, btn('Save', () => cmd('SaveActivePreset')), btn('Rename', prompter('New preset name', (n) => cmd('RenameActivePreset', n))),
          dyn((s) => s.files.presets, (p) => sel(null, [['', 'Load preset file...'], ...p.map((x) => [x, x])], () => '', (v) => v && cmd('LoadEffectPreset', v))))),
      fx('Reverb', null, st('reverb.style', 'SetReverbStyle', ['Library', 'DarkBloom', 'MusicClub', 'RealPlate', 'Chapel', 'HockeyArena']),
        S1('Amount', 'reverb.amount', 'SetReverbAmount', 0, 100, '%'), S1('Decay', 'reverb.decay', 'SetReverbDecay', 10, 20000, ' ms'),
        S1('Early level', 'reverb.early_level', 'SetReverbEarlyLevel', -25, 0, ' dB'), S1('Tail level', 'reverb.tail_level', 'SetReverbTailLevel', -25, 0, ' dB'),
        S1('Pre-delay', 'reverb.pre_delay', 'SetReverbPreDelay', 0, 100, ' ms'), S1('Low colour', 'reverb.lo_colour', 'SetReverbLowColour', -50, 50),
        S1('High colour', 'reverb.hi_colour', 'SetReverbHighColour', -50, 50), S1('High factor', 'reverb.hi_factor', 'SetReverbHighFactor', -25, 25),
        S1('Diffuse', 'reverb.diffuse', 'SetReverbDiffuse', -50, 50), S1('Mod speed', 'reverb.mod_speed', 'SetReverbModSpeed', -25, 25), S1('Mod depth', 'reverb.mod_depth', 'SetReverbModDepth', -25, 25)),
      fx('Echo', null, st('echo.style', 'SetEchoStyle', ['Quarter', 'Eighth', 'Triplet', 'PingPong', 'ClassicSlap', 'MultiTap']),
        S1('Amount', 'echo.amount', 'SetEchoAmount', 0, 100, '%'), S1('Feedback', 'echo.feedback', 'SetEchoFeedback', 0, 100, '%'), S1('Tempo', 'echo.tempo', 'SetEchoTempo', 45, 300, ' bpm'),
        S1('Delay L', 'echo.delay_left', 'SetEchoDelayLeft', 0, 2000, ' ms'), S1('Delay R', 'echo.delay_right', 'SetEchoDelayRight', 0, 2000, ' ms'),
        S1('Feedback L', 'echo.feedback_left', 'SetEchoFeedbackLeft', 0, 100, '%'), S1('Feedback R', 'echo.feedback_right', 'SetEchoFeedbackRight', 0, 100, '%'),
        S1('XFB L to R', 'echo.feedback_xfb_l_to_r', 'SetEchoFeedbackXFBLtoR', 0, 100, '%'), S1('XFB R to L', 'echo.feedback_xfb_r_to_l', 'SetEchoFeedbackXFBRtoL', 0, 100, '%')),
      fx('Pitch', null, st('pitch.style', 'SetPitchStyle', ['Narrow', 'Wide']), S1('Amount', 'pitch.amount', 'SetPitchAmount', -24, 24), S1('Character', 'pitch.character', 'SetPitchCharacter', 0, 100)),
      fx('Gender', null, st('gender.style', 'SetGenderStyle', ['Narrow', 'Medium', 'Wide']), S1('Amount', 'gender.amount', 'SetGenderAmount', -12, 12))),
      h('div', { class: 'col' },
      fx('Megaphone', toggle('Enabled', (m) => m.effects.current.megaphone.is_enabled, (v) => cmd('SetMegaphoneEnabled', v)),
        st('megaphone.style', 'SetMegaphoneStyle', ['Megaphone', 'Radio', 'OnThePhone', 'Overdrive', 'BuzzCutt', 'Tweed']),
        S1('Amount', 'megaphone.amount', 'SetMegaphoneAmount', 0, 100, '%'), S1('Post gain', 'megaphone.post_gain', 'SetMegaphonePostGain', -20, 20, ' dB')),
      fx('Hard tune', toggle('Enabled', (m) => m.effects.current.hard_tune.is_enabled, (v) => cmd('SetHardTuneEnabled', v)),
        st('hard_tune.style', 'SetHardTuneStyle', ['Natural', 'Medium', 'Hard']),
        S1('Amount', 'hard_tune.amount', 'SetHardTuneAmount', 0, 100, '%'), S1('Rate', 'hard_tune.rate', 'SetHardTuneRate', 0, 100), S1('Window', 'hard_tune.window', 'SetHardTuneWindow', 0, 600, ' ms'),
        sel('Source', ['All', 'Music', 'Game', 'LineIn', 'System'].map((o) => [o, label(o)]), E('hard_tune.source'), (v) => cmd('SetHardTuneSource', v))),
      fx('Robot', toggle('Enabled', (m) => m.effects.current.robot.is_enabled, (v) => cmd('SetRobotEnabled', v)),
        st('robot.style', 'SetRobotStyle', ['Robot1', 'Robot2', 'Robot3']),
        ...['Low', 'Medium', 'High'].flatMap((r) => { const k = { Low: 'low', Medium: 'mid', High: 'high' }[r]; return [
          h('div', { class: 'sub' }, r + ' band'), S1(r + ' gain', `robot.${k}_gain`, 'SetRobotGain', -12, 12, ' dB', [r]), S1(r + ' freq', `robot.${k}_freq`, 'SetRobotFreq', 0, 88, '', [r]), S1(r + ' width', `robot.${k}_width`, 'SetRobotWidth', 0, 88, '', [r])]; }),
        h('div', { class: 'sub' }, 'Voice'), S1('Waveform', 'robot.waveform', 'SetRobotWaveform', 0, 2), S1('Pulse width', 'robot.pulse_width', 'SetRobotPulseWidth', 0, 100),
        S1('Threshold', 'robot.threshold', 'SetRobotThreshold', -36, 0, ' dB'), S1('Dry mix', 'robot.dry_mix', 'SetRobotDryMix', -36, 0, ' dB'))));
  },

  sampler() {
    const m = mixer();
    if (!m.sampler) return h('div', { class: 'empty' }, 'Sampler is not available on this device.');
    const MODES = ['PlayNext', 'PlayStop', 'PlayFade', 'StopOnRelease', 'FadeOnRelease', 'Loop'];
    const pads = h('div', { class: 'pads' });
    const body = dyn((s, m) => m.sampler.active_bank, (bank) => {
      pads.replaceChildren();
      for (const b of ['TopLeft', 'TopRight', 'BottomLeft', 'BottomRight']) {
        const pad = h('div', { class: 'pad' }, h('h4', {}, label(b)));
        bind(pad, (m) => m.sampler.banks[bank][b].is_playing, (el, v) => el.classList.toggle('playing', !!v));
        pad.append(sel('Mode', MODES.map((x) => [x, label(x)]), (m) => m.sampler.banks[bank][b].function, (v) => cmd('SetSamplerFunction', bank, b, v)),
          seg('Order', ['Sequential', 'Random'], (m) => m.sampler.banks[bank][b].order, (v) => cmd('SetSamplerOrder', bank, b, v)),
          dyn((s, m) => m.sampler.banks[bank][b].samples, (samples) => samples.length ? [h('div', { class: 'sample hd' }, h('span', { class: 'n' }, 'Sample'), h('span', {}, 'Start %'), h('span', {}, 'Stop %'), h('span', {})), ...samples.map((smp, i) => h('div', { class: 'sample' },
            h('span', { class: 'n', title: smp.name }, smp.name),
            h('input', { type: 'number', min: 0, max: 100, step: 0.1, value: smp.start_pct, title: 'Start %', onchange: (e) => cmd('SetSampleStartPercent', bank, b, i, +e.target.value) }),
            h('input', { type: 'number', min: 0, max: 100, step: 0.1, value: smp.stop_pct, title: 'Stop %', onchange: (e) => cmd('SetSampleStopPercent', bank, b, i, +e.target.value) }),
            h('span', { class: 'acts' }, btn('Play', () => cmd('PlaySampleByIndex', bank, b, i)), h('button', { class: 'b danger', title: 'Remove sample', 'aria-label': 'Remove sample', onclick: () => cmd('RemoveSampleByIndex', bank, b, i) }, '\u2715'))))] : h('p', { class: 'muted' }, 'No samples')),
          h('div', { class: 'row' }, dyn((s) => Object.keys(s.files.samples), (names) => sel(null, [['', 'Add sample...'], ...names.map((n) => [n, n])], () => '', (v) => v && cmd('AddSample', bank, b, v))),
            btn('Stop', () => cmd('StopSamplePlayback', bank, b))));
        pads.append(pad);
      }
      return pads;
    });
    return h('div', { class: 'grid', style: 'grid-template-columns:minmax(0,1fr)' },
      dyn((s, m) => m.sampler.processing_state.last_error, (e) => e ? h('div', { class: 'alert', role: 'alert' }, h('span', {}, '\u26a0 Sample error: ' + e), btn('Dismiss', () => cmd('ClearSampleProcessError'))) : null),
      card('Sampler', seg('Bank', ['A', 'B', 'C'], (m) => m.sampler.active_bank, (v) => cmd('SetActiveSamplerBank', v)), body),
      card('Sampler settings', toggle('Reset on clear', (m) => m.settings.reset_sampler_on_clear, (v) => cmd('SetSamplerResetOnClear', v)),
        slider('Fade time', { min: 0, max: 5000, step: 50, get: (m) => m.settings.fade_duration, set: (v) => cmd('SetSamplerFadeDuration', v), fmt: (v) => v + ' ms' }),
        slider('Record buffer', { min: 0, max: 30000, step: 500, get: (m) => m.sampler.record_buffer, set: (v) => cmd('SetSamplerPreBufferDuration', v), fmt: (v) => v / 1000 + ' s' }),
      ),
      card('Sample files', dyn((s) => s.files.samples, (files) => Object.entries(files).map(([k, f]) => h('div', { class: 'row' },
        h('label', { style: 'flex:1;color:var(--text);min-width:100px' }, f.name || k), h('audio', { controls: true, preload: 'none', src: '/files/samples/' + encodeURIComponent(k) }),
        h('label', { style: 'min-width:0;flex:0 0 auto;color:var(--mute)' }, 'Gain %'), h('input', { type: 'number', min: 0, max: 100, value: f.gain_pct, 'aria-label': 'Gain %', style: 'width:70px', onchange: (e) => daemon('SetSampleGainPct', k, +e.target.value) }))))));
  },

  lighting() {
    const m = mixer();
    const L = m.lighting;
    const hex = (c) => '#' + String(c).replace('#', '').padEnd(6, '0').slice(0, 6);
    const colour = (get, set) => { const i = h('input', { type: 'color', 'aria-label': 'Colour', title: 'Pick colour' }); i.addEventListener('change', () => set(i.value.slice(1).toUpperCase())); return bind(i, get, (el, v) => (el.value = hex(v))); };
    const cards = [];
    if (L.animation.supported) cards.push(card('Animation',
      seg('Mode', ['RetroRainbow', 'RainbowDark', 'RainbowBright', 'Simple', 'Ripple', 'None'], (m) => m.lighting.animation.mode, (v) => cmd('SetAnimationMode', v)),
      slider('Mod 1', { min: 0, max: 255, get: (m) => m.lighting.animation.mod1, set: (v) => cmd('SetAnimationMod1', v) }),
      slider('Mod 2', { min: 0, max: 255, get: (m) => m.lighting.animation.mod2, set: (v) => cmd('SetAnimationMod2', v) }),
      seg('Waterfall', ['Down', 'Up', 'Off'], (m) => m.lighting.animation.waterfall_direction, (v) => cmd('SetAnimationWaterfall', v))));
    cards.push(card('Global', row('Global', colour((m) => m.lighting.simple.Global.colour_one, (v) => cmd('SetGlobalColour', v))),
      ...Object.keys(L.simple).filter((k) => k !== 'Global').map((k) => row(label(k), colour((m) => m.lighting.simple[k].colour_one, (v) => cmd('SetSimpleColour', k, v))))));
    cards.push(card('Faders', row('All', colour(() => '000000', (v) => cmd('SetAllFaderColours', v, v)), sel(null, ['TwoColour', 'Gradient', 'Meter', 'GradientMeter'].map((o) => [o, label(o)]), () => '', (v) => v && cmd('SetAllFaderDisplayStyle', v))),
      ...Object.keys(L.faders).map((f) => row('Fader ' + f, colour((m) => m.lighting.faders[f].colours.colour_one, (v) => cmd('SetFaderColours', f, v, mixer().lighting.faders[f].colours.colour_two)),
        colour((m) => m.lighting.faders[f].colours.colour_two, (v) => cmd('SetFaderColours', f, mixer().lighting.faders[f].colours.colour_one, v)),
        sel(null, ['TwoColour', 'Gradient', 'Meter', 'GradientMeter'].map((o) => [o, label(o)]), (m) => m.lighting.faders[f].style, (v) => cmd('SetFaderDisplayStyle', f, v))))));
    cards.push(card('Buttons', ...Object.keys(L.buttons).map((b) => row(label(b), colour((m) => m.lighting.buttons[b].colours.colour_one, (v) => cmd('SetButtonColours', b, v, mixer().lighting.buttons[b].colours.colour_two)),
      colour((m) => m.lighting.buttons[b].colours.colour_two, (v) => cmd('SetButtonColours', b, mixer().lighting.buttons[b].colours.colour_one, v)),
      sel(null, ['Dimmed', 'Colour2', 'DimmedColour2'].map((o) => [o, label(o)]), (m) => m.lighting.buttons[b].off_style, (v) => cmd('SetButtonOffStyle', b, v))))));
    if (Object.keys(L.encoders).length) cards.push(card('Encoders', ...Object.keys(L.encoders).map((e) => row(e, ...['colour_one', 'colour_two', 'colour_three'].map((k) => colour((m) => m.lighting.encoders[e][k], (v) => { const c = { ...mixer().lighting.encoders[e], [k]: v }; cmd('SetEncoderColour', e, c.colour_one, c.colour_two, c.colour_three); }))))));
    if (Object.keys(L.sampler).length) cards.push(card('Sampler banks', ...Object.keys(L.sampler).map((s) => row(label(s), ...['colour_one', 'colour_two', 'colour_three'].map((k) => colour((m) => m.lighting.sampler[s].colours[k], (v) => { const c = { ...mixer().lighting.sampler[s].colours, [k]: v }; cmd('SetSampleColour', s, c.colour_one, c.colour_two, c.colour_three); })),
      sel(null, ['Dimmed', 'Colour2', 'DimmedColour2'].map((o) => [o, label(o)]), (m) => m.lighting.sampler[s].off_style, (v) => cmd('SetSampleOffStyle', s, v))))));
    return h('div', { class: 'masonry' }, ...cards);
  },

  profiles() {
    const listCard = (title, filesKey, curKey, ops) => card(title,
      dyn((s, m) => [s.files[filesKey], m[curKey]], ([files, cur]) => files.map((n) => h('div', { class: 'row' },
        h('label', { style: 'flex:1;color:var(--text);min-width:0' }, n, n === cur ? h('span', { class: 'badge', style: 'margin-left:10px' }, 'Active') : null),
        (() => { const l = btn('Load', () => ops.load(n)); l.disabled = n === cur; return l; })(), btn('Delete', () => confirm(`Delete "${n}"?`) && ops.del(n), 'danger')))),
      h('div', { class: 'row' }, btn('Save', ops.save), btn('Save as...', prompter('Save as', ops.saveAs)), btn('New...', prompter('New name', ops.create))));
    return h('div', { class: 'grid' },
      listCard('Profiles', 'profiles', 'profile_name', { load: (n) => cmd('LoadProfile', n, true), del: (n) => cmd('DeleteProfile', n), save: () => cmd('SaveProfile'), saveAs: (n) => cmd('SaveProfileAs', n), create: (n) => cmd('NewProfile', n) }),
      listCard('Mic profiles', 'mic_profiles', 'mic_profile_name', { load: (n) => cmd('LoadMicProfile', n, true), del: (n) => cmd('DeleteMicProfile', n), save: () => cmd('SaveMicProfile'), saveAs: (n) => cmd('SaveMicProfileAs', n), create: (n) => cmd('NewMicProfile', n) }),
      h('div', { class: 'card', style: 'grid-column:1/-1' }, h('h3', {}, 'Folders'), ...['Profiles', 'MicProfiles', 'Presets', 'Samples', 'Icons', 'Logs', 'Backups'].map((p) => h('div', { class: 'row' }, h('label', { style: 'flex:1;color:var(--text);min-width:0' }, p), btn('Open', () => daemon('OpenPath', p)), btn('Restore defaults', () => confirm(`Restore default ${p}?`) && daemon('RecoverDefaults', p))))));
  },

  settings() {
    const m = mixer();
    const dt = (label_, get, set) => h('div', { class: 'row' }, toggle(label_, get, set));
    const D = (k) => () => S.status.config[k];
    const hw = m.hardware;
    return h('div', { class: 'masonry' },
      card('Mixer', toggle('VC mute also mutes cough', (m) => m.settings.vc_mute_also_mute_cm, (v) => cmd('SetVCMuteAlsoMuteCM', v)),
        slider('Mute hold', { min: 0, max: 5000, step: 50, get: (m) => m.settings.mute_hold_duration, set: (v) => cmd('SetMuteHoldDuration', v), fmt: (v) => v + ' ms' }),
        seg('VOD mode', ['Routable', 'StreamNoMusic'], (m) => m.settings.vod_mode, (v) => cmd('SetVodMode', v)),
        h('div', { class: 'sub' }, 'Hardware display mode'),
        ...[['NoiseGate', 'gate'], ['Equaliser', 'equaliser'], ['Compressor', 'compressor'], ['EqFineTune', 'equaliser_fine']].map(([c, k]) => seg(c, ['Simple', 'Advanced'], (m) => m.settings.display[k], (v) => cmd('SetElementDisplayMode', c, v)))),
      card('Daemon',
        dt('Show tray icon', D('show_tray_icon'), (v) => daemon('SetShowTrayIcon', v)),
        dt('Start on login', D('autostart_enabled'), (v) => daemon('SetAutoStartEnabled', v)),
        dt('Open UI on launch', D('open_ui_on_launch'), (v) => daemon('SetUiLaunchOnLoad', v)),
        dt('Allow network access', D('allow_network_access'), (v) => daemon('SetAllowNetworkAccess', v)),
        h('p', { class: 'hint' }, 'Lets other computers on your network open this UI and control the mixer.'),
        S.status.config.tts_enabled !== null && S.status.config.tts_enabled !== undefined ? dt('Text to speech', D('tts_enabled'), (v) => daemon('SetTTSEnabled', v)) : null,
        sel('Log level', ['Off', 'Error', 'Warn', 'Info', 'Debug', 'Trace'], () => S.status.config.log_level, (v) => daemon('SetLogLevel', v)),
        sel('Firmware', [['Live', 'Live'], ['Beta', 'Beta']], () => S.status.config.firmware_source, (v) => daemon('SetFirmwareSource', v)),
        h('div', { class: 'sub' }, 'Danger zone'), h('div', { class: 'row' }, btn('Stop daemon', () => confirm('Stop the GoXLR daemon?') && daemon('StopDaemon'), 'danger'))),
      card('Device', ...[['Model', hw.device_type], ['Serial', hw.serial_number], ['Manufactured', hw.manufactured_date], ['Firmware', hw.versions.firmware?.join?.('.') ?? JSON.stringify(hw.versions.firmware)], ['Daemon', S.status.config.daemon_version], ['Platform', S.status.config.platform]].map(([k, v]) => row(k, h('span', {}, String(v))))));
  },
};

const NAV = [['mixer', 'Mixer'], ['routing', 'Routing'], ['mic', 'Microphone'], ['effects', 'Effects'], ['sampler', 'Sampler'], ['lighting', 'Lighting'], ['profiles', 'Profiles'], ['settings', 'Settings']];

function txtInput(ph, get, set) {
  const i = h('input', { type: 'text', placeholder: ph, style: 'width:90px' });
  i.addEventListener('change', () => set(i.value));
  return bind(i, get, (el, v) => { if (document.activeElement !== el) el.value = v; });
}

function pollMic(cb) {
  clearInterval(micTimer);
  micTimer = setInterval(async () => {
    if (!S.online || !S.serial || document.hidden) return;
    const r = await send({ GetMicLevel: S.serial });
    if (r && r.MicLevel !== undefined) cb(r.MicLevel);
  }, 120);
}

const SUN = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg>';
const MOON = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"/></svg>';
const curTheme = () => document.documentElement.dataset.theme || (matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark');
function themeButton(extra = '') {
  const b = h('button', { class: ('theme ' + extra).trim(), title: 'Toggle dark mode', 'aria-label': 'Toggle dark mode' });
  const paint = () => { b.innerHTML = (curTheme() === 'dark' ? SUN : MOON) + '<span>' + (curTheme() === 'dark' ? 'Light mode' : 'Dark mode') + '</span>'; };
  b.onclick = () => { const n = curTheme() === 'dark' ? 'light' : 'dark'; document.documentElement.dataset.theme = n; try { localStorage.setItem('theme', n); } catch { /* storage blocked */ } paint(); };
  paint();
  return b;
}

function renderTop() {
  const t = $('#top');
  const m = S.status?.mixers;
  t.replaceChildren(...[
    h('h1', {}, NAV.find((n) => n[0] === S.view)[1]),
    m && Object.keys(m).length > 1 ? sel(null, Object.keys(m).map((s) => [s, `${m[s].hardware.device_type} ${s}`]), () => S.serial, (v) => { S.serial = v; render(); }) : null,
    mixer() ? h('span', { class: 'muted' }, `Profile: ${mixer().profile_name}  \u00b7  Mic: ${mixer().mic_profile_name}`) : null,
    h('span', {}, h('i', { class: 'dot' + (S.online ? ' ok' : '') }), S.online ? 'Connected' : 'Offline'),
    themeButton('top-theme'),
  ].filter(Boolean));
  document.querySelectorAll('#nav button[data-v]').forEach((b) => b.classList.toggle('on', b.dataset.v === S.view));
}

function render() {
  bound = [];
  clearInterval(micTimer);
  const nav = $('#nav');
  nav.replaceChildren(h('div', { class: 'brand' }, h('img', { src: '/icon.svg', alt: '', width: 28, height: 28 }), h('span', {}, 'GoXLR ', h('b', {}, 'Utility'))),
    ...NAV.map(([v, t]) => h('button', { 'data-v': v, onclick: () => { S.view = v; try { location.hash = v; } catch { /* ignore */ } render(); } }, t)),
    h('div', { class: 'sp' }), themeButton());
  renderTop();
  nav.querySelector('button.on')?.scrollIntoView?.({ inline: 'center', block: 'nearest' });
  const view = $('#view');
  if (!S.status) return view.replaceChildren(h('div', { class: 'empty' }, 'Connecting to daemon...'));
  if (!mixer()) return view.replaceChildren(h('div', { class: 'empty' }, 'No GoXLR detected. Plug in a device.'));
  view.replaceChildren(views[S.view]());
  refresh();
}

try { document.documentElement.dataset.theme = localStorage.getItem('theme') || ''; } catch { /* storage blocked */ }
if (!document.documentElement.dataset.theme) delete document.documentElement.dataset.theme;
if (views[location.hash.slice(1)]) S.view = location.hash.slice(1);
render();
connect();
