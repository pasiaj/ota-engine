/* Visual identity for every OTA component: colour, glyph (48×48 SVG, stroke = currentColor),
 * family, shape and a one-line "motif" that explains the identity.
 * Used by every page so a component looks the same wherever it appears. */
(function () {
  'use strict';
  const FAMILIES = {
    phase:     { name: 'OTA phase',          shape: 'circle',  desc: 'The three muscles of the loop' },
    loop:      { name: 'Loop',               shape: 'ring',    desc: 'The cycle and its compounding' },
    modality:  { name: 'Modality',           shape: 'square',  desc: 'Five dimensions where the loop holds or breaks' },
    pathogen:  { name: 'Pathogen',           shape: 'virus',   desc: 'Diseases that infect the loop from inside' },
    tool:      { name: 'Human tool',         shape: 'hex',     desc: 'Part II toolkit: practices people run' },
    ai:        { name: 'AI-native tool',     shape: 'diamond', desc: 'Part IV: the same logic on a machine engine' },
    principle: { name: 'Principle',          shape: 'star',    desc: 'Stances and design rules' },
    failure:   { name: 'Failure mode',       shape: 'tri',     desc: 'Named ways the loop goes wrong' },
    zone:      { name: 'Grid zone',          shape: 'square',  desc: 'Named regions of the success grid' }
  };

  // glyph helpers
  const P = d => `<path d="${d}"/>`;
  const C = (x, y, r, f) => `<circle cx="${x}" cy="${y}" r="${r}"${f ? ' fill="currentColor"' : ''}/>`;

  const C_ = {
    // ---- phases
    observe:   { n: 'Observe', fi: 'Havaitse', fam: 'phase', col: '#3fd0c9', motif: 'An eye with a radar sweep: seeing reality before others do.',
      g: P('M4 24c6-9 13-14 20-14s14 5 20 14c-6 9-13 14-20 14S10 33 4 24z') + C(24, 24, 7) + C(24, 24, 2.5, 1) + P('M24 17v-5M31 24h5') },
    think:     { n: 'Think', fi: 'Ajattele', fam: 'phase', col: '#a98bff', motif: 'A prism splitting one signal into several hypotheses.',
      g: P('M24 6 40 36H8z') + P('M2 22l14 4') + P('M30 26l16-8M31 28l15 0M30 30l16 8') },
    act:       { n: 'Act', fi: 'Toimi', fam: 'phase', col: '#ff9a3d', motif: 'A vector leaving a gate: insight turned into momentum.',
      g: P('M8 10v28') + P('M14 24h24M30 16l8 8-8 8') + P('M14 14h6M14 34h6') },
    loop:      { n: 'OTA loop', fi: 'OTA-silmukka', fam: 'loop', col: '#f1d27a', motif: 'Three arcs chasing each other: never a one-off process.',
      g: P('M24 6a18 18 0 0 1 15.6 9') + P('M41 30A18 18 0 0 1 24 42') + P('M12 37A18 18 0 0 1 8 16') + P('M36 10l4 5-6 1M28 44l-5-2 4-4M6 20l2-5 4 4') },
    flywheel:  { n: 'OTA flywheel', fi: 'OTA-vauhtipyörä', fam: 'loop', col: '#f1d27a', motif: 'A spoked wheel with momentum trails: every turn cheaper and sharper.',
      g: C(24, 24, 16) + C(24, 24, 4, 1) + P('M24 8v32M8 24h32M12.7 12.7l22.6 22.6M35.3 12.7 12.7 35.3') + P('M44 14a22 22 0 0 1 0 20') },
    // ---- modalities
    direction: { n: 'Direction', fi: 'Suunta', fam: 'modality', col: '#ff5d5d', motif: 'A compass needle: where the organisation is heading.',
      g: C(24, 24, 18) + P('M24 10l5 14-5 14-5-14z') + C(24, 24, 2, 1) },
    structure: { n: 'Structure', fi: 'Rakenne', fam: 'modality', col: '#6aa8ff', motif: 'An org lattice: who decides, and how it is organised.',
      g: '<rect x="18" y="6" width="12" height="9" rx="1"/><rect x="5" y="33" width="12" height="9" rx="1"/><rect x="31" y="33" width="12" height="9" rx="1"/>' + P('M24 15v9M11 24h26M11 24v9M37 24v9') },
    processes: { n: 'Processes', fi: 'Prosessit', fam: 'modality', col: '#5fd38d', motif: 'Interlocking gears: how work is done every day.',
      g: C(18, 20, 8) + C(18, 20, 2.5) + C(33, 32, 6) + C(33, 32, 2) + P('M18 9v3M18 28v3M7 20h3M26 20h3M33 23v2M33 39v2M24 32h2M40 32h2') },
    capability:{ n: 'Capability', fi: 'Kyvykkyys', fam: 'modality', col: '#ffc94a', motif: 'A cut gem: what the organisation can really do.',
      g: P('M10 18l7-9h14l7 9-14 22z') + P('M10 18h28M17 9l7 9 7-9M24 18v22') },
    culture:   { n: 'Culture', fi: 'Kulttuuri', fam: 'modality', col: '#e276d6', motif: 'A flame over roots: how people behave when no one is watching.',
      g: P('M24 6c6 7 10 11 10 17a10 10 0 0 1-20 0c0-4 2-7 5-9 0 4 2 6 4 6-2-5 0-10 1-14z') + P('M24 34v8M24 38l-7 5M24 38l7 5') },
    // ---- pathogens
    hubris:    { n: 'Hubris', fi: 'Hybris', fam: 'pathogen', col: '#e8c34a', motif: 'A crown on a cracked map: the model mistaken for reality.',
      g: P('M10 20l5 6 9-12 9 12 5-6-3 14H13z') + P('M14 40h20') + P('M22 26l2 4-2 4') },
    complacency:{ n: 'Complacency', fi: 'Itsetyytyväisyys', fam: 'pathogen', col: '#9ccf6a', motif: 'A closed eye under a flat line: nothing went wrong lately.',
      g: P('M6 24c6 6 12 8 18 8s12-2 18-8') + P('M12 30l-3 4M24 32v5M36 30l3 4') + P('M8 14h32') },
    'normalisation-of-deviance': { n: 'Normalisation of deviance', fi: 'Poikkeamien normalisoituminen', fam: 'pathogen', col: '#c9a0ff', motif: 'A tolerance band that creeps a little wider with every safe exception.',
      g: P('M4 30h8l4-4 4 3 4-6 4 4 4-8 4 5 4-10 4 2') + P('M4 36c14 0 26-6 40-18') + P('M4 40h40') },
    groupthink:{ n: 'Groupthink', fi: 'Ryhmäajattelu', fam: 'pathogen', col: '#ff8fb1', motif: 'Many heads, one arrow: the set of models shrinks under social pressure.',
      g: C(12, 16, 4) + C(24, 13, 4) + C(36, 16, 4) + P('M6 30c2-6 10-6 12 0M18 27c2-6 10-6 12 0M30 30c2-6 10-6 12 0') + P('M8 40h30M32 36l6 4-6 4') },
    'misleading-incentives': { n: 'Misleading incentives', fi: 'Harhaanjohtavat kannustimet', fam: 'pathogen', col: '#ffa35c', motif: 'A target with the arrow in the wrong ring: optimising what is paid, not what is needed.',
      g: C(20, 26, 14) + C(20, 26, 8) + C(20, 26, 2, 1) + P('M44 6 30 18M38 6h6v6') },
    inertia:   { n: 'Inertia', fi: 'Inertia', fam: 'pathogen', col: '#7fb2c9', motif: 'A heavy block on rails, accelerating straight past the turn.',
      g: '<rect x="6" y="14" width="22" height="16" rx="2"/>' + P('M2 36h44M30 22h10M34 18l6 4-6 4') + P('M40 36c3-4 4-10 4-16') },
    // ---- human tools (Part II)
    'anomaly-diary': { n: 'Anomaly diary', fi: 'Poikkeamapäiväkirja', fam: 'tool', ph: 'O', col: '#3fd0c9', motif: 'A notebook with one entry that refuses to fit the line.',
      g: '<rect x="10" y="6" width="28" height="36" rx="2"/>' + P('M16 15h16M16 21h16M16 33h16') + C(30, 27, 2.6, 1) },
    'observation-stack': { n: 'Observation stack', fi: 'Havaintopino', fam: 'tool', ph: 'O', col: '#3fd0c9', motif: 'Layers condensing raw signal into direction.',
      g: P('M8 38h32M11 31h26M14 24h20M17 17h14M20 10h8') },
    'reality-instrumentation': { n: 'Reality instrumentation', fi: 'Todellisuuden monitorointi', fam: 'tool', ph: 'O', col: '#3fd0c9', motif: 'A gauge aimed at the variable that matters, not the easy one.',
      g: P('M8 34a16 16 0 0 1 32 0') + P('M24 34l9-12') + C(24, 34, 2.5, 1) + P('M12 26l2 1M24 18v2M36 26l-2 1') },
    'instrument-audit': { n: 'Instrument audit', fi: 'Mittaristoauditointi', fam: 'tool', ph: 'O', col: '#3fd0c9', motif: 'A magnifier over the dashboard itself.',
      g: '<rect x="5" y="8" width="26" height="20" rx="2"/>' + P('M10 22l5-5 4 3 6-7') + C(32, 30, 7) + P('M37 35l7 7') },
    triangulation: { n: 'Triangulation', fi: 'Triangulaatio', fam: 'tool', ph: 'O', col: '#3fd0c9', motif: 'Three independent bearings converging on one point.',
      g: C(8, 38, 3) + C(40, 38, 3) + C(24, 8, 3) + P('M10 36l12-10M38 36 26 26M24 11v13') + C(24, 25, 2, 1) },
    'observation-review': { n: 'Observation review', fi: 'Tilannekuvakatsaus', fam: 'tool', ph: 'O', col: '#3fd0c9', motif: 'A calendar beat where signals meet expectations.',
      g: '<rect x="7" y="10" width="34" height="30" rx="2"/>' + P('M7 18h34M15 6v8M33 6v8') + P('M14 30l5 4 13-10') },
    'thinking-pipeline': { n: 'Thinking pipeline', fi: 'Ajatteluketju', fam: 'tool', ph: 'T', col: '#a98bff', motif: 'Six linked chambers from facts to the breaking point.',
      g: C(6, 24, 3) + C(15, 24, 3) + C(24, 24, 3) + C(33, 24, 3) + C(42, 24, 3, 1) + P('M9 24h3M18 24h3M27 24h3M36 24h3') + P('M4 32h40') },
    'pre-mortem': { n: 'Pre-mortem', fi: 'Ennakkoarviointi', fam: 'tool', ph: 'T', col: '#a98bff', motif: 'A timeline folded back: imagine it already failed.',
      g: P('M6 34h28a8 8 0 0 0 0-16H14') + P('M18 13l-5 5 5 5') + P('M36 30l4 4m0-4-4 4') },
    wwhtbt: { n: 'What would have to be true', fi: 'Minkä pitäisi olla totta (MPOT)', fam: 'tool', ph: 'T', col: '#a98bff', motif: 'A question mark resting on explicit conditions.',
      g: P('M17 14a7 7 0 1 1 10 6c-2 1-3 3-3 5v2') + C(24, 32, 1.6, 1) + P('M8 40h32M12 40v-3M24 40v-3M36 40v-3') },
    'red-team': { n: 'Red team', fi: 'Punainen tiimi', fam: 'tool', ph: 'T', col: '#ff6b6b', motif: 'Crossed blades: the strongest serious counter-case.',
      g: P('M8 8l26 26M40 8 14 34') + P('M30 38l8-8M10 30l8 8') },
    'scenario-mapping': { n: 'Scenario mapping', fi: 'Skenaariokartoitus', fam: 'tool', ph: 'T', col: '#a98bff', motif: 'A trunk with open branches: keep more than one future alive.',
      g: P('M24 44V26M24 26 12 12M24 26l12-14M24 32l-12 4M24 30l10 2') + C(12, 12, 2.5, 1) + C(36, 12, 2.5, 1) + C(12, 36, 2, 1) + C(34, 32, 2, 1) },
    'core-competence': { n: 'Core competence identification', fi: 'Ydinkyvykkyyden tunnistaminen', fam: 'tool', ph: 'T', col: '#ffc94a', motif: 'Three circles; only the overlap counts.',
      g: C(18, 19, 10) + C(30, 19, 10) + C(24, 30, 10) + C(24, 23, 2.4, 1) },
    calibration: { n: 'Calibration', fi: 'Kalibrointi', fam: 'tool', ph: 'T', col: '#a98bff', motif: 'A diagonal of perfect forecasts and the dots that drift from it.',
      g: P('M6 42h36M6 42V6') + P('M8 40 40 8') + C(14, 32, 1.8, 1) + C(22, 26, 1.8, 1) + C(30, 14, 1.8, 1) + C(36, 16, 1.8, 1) },
    'decision-record': { n: 'Decision record', fi: 'Päätöskirjaus', fam: 'tool', ph: 'A', col: '#ff9a3d', motif: 'A sealed document opened before, closed after.',
      g: P('M12 6h18l8 8v28H12z') + P('M30 6v8h8') + P('M17 22h14M17 28h14M17 34h8') },
    'after-action-review': { n: 'After-action review', fi: 'Toiminnan jälkiarviointi', fam: 'tool', ph: 'A', col: '#ff9a3d', motif: 'Four quadrants: expected, happened, why, next.',
      g: '<rect x="6" y="6" width="36" height="36" rx="3"/>' + P('M24 6v36M6 24h36') + P('M34 30a5 5 0 1 1-4 8') },
    'domain-expert-sign-off': { n: 'Domain expert sign-off', fi: 'Asiantuntijahyväksyntä', fam: 'tool', ph: 'A', col: '#ff9a3d', motif: 'A gate stamped by someone with no stake.',
      g: P('M6 42V12h36v30') + P('M6 20h36') + C(24, 31, 7) + P('M21 31l2 2 4-4') },
    'direction-alignment': { n: 'Direction alignment (OKR)', fi: 'Suunnan linjaus', fam: 'tool', ph: 'A', col: '#ff9a3d', motif: 'Many small arrows snapping to one heading.',
      g: P('M6 12h20M20 8l6 4-6 4M6 24h30M30 20l6 4-6 4M6 36h24M24 32l6 4-6 4') + P('M42 6v36') },
    reversibility: { n: 'Reversibility & impact', fi: 'Peruttavuus', fam: 'principle', ph: 'A', col: '#ff9a3d', motif: 'A two-way door vs. a one-way door.',
      g: '<rect x="6" y="8" width="14" height="32" rx="1"/><rect x="28" y="8" width="14" height="32" rx="1"/>' + P('M9 24h8M14 20l3 4-3 4M12 20l-3 4 3 4M31 24h8M36 20l3 4-3 4') },
    // ---- AI-native tools (Part IV)
    'reality-lattice': { n: 'Reality lattice', fi: 'Todellisuuskudos', fam: 'ai', ph: 'O', col: '#3fd0c9', motif: 'A six-level mesh woven from raw data up to prioritised insight.',
      g: P('M6 40h36M10 33h28M14 26h20M18 19h12M22 12h4') + P('M6 40 22 12M42 40 26 12M14 40l10-28M34 40 24 12') },
    'signal-brief': { n: 'Signal brief', fi: 'Signaalikatsaus', fam: 'ai', ph: 'O', col: '#3fd0c9', motif: 'A one-page digest with a pen: AI drafts, human signs.',
      g: '<rect x="8" y="6" width="26" height="34" rx="2"/>' + P('M13 14h16M13 20h16M13 26h10') + P('M30 40l12-12 3 3-12 12h-3z') },
    'decision-ledger': { n: 'Decision ledger', fi: 'Päätöskirjanpito', fam: 'ai', ph: 'T', col: '#a98bff', motif: 'An append-only chain of sealed blocks.',
      g: '<rect x="4" y="16" width="12" height="16" rx="2"/><rect x="18" y="16" width="12" height="16" rx="2"/><rect x="32" y="16" width="12" height="16" rx="2"/>' + P('M16 24h2M30 24h2') + P('M8 22h4M22 22h4M36 22h4') },
    'calibration-tracker': { n: 'Calibration tracker', fi: 'Kalibroinninseuranta', fam: 'ai', ph: 'T', col: '#a98bff', motif: 'A reliability curve with a live Brier score.',
      g: P('M6 42h36M6 42V6M8 40 40 8') + P('M8 38c8-2 14-10 18-16s8-10 14-12') + C(40, 10, 2.5, 1) },
    'assumption-cycle': { n: 'Assumption cycle', fi: 'Oletussykli', fam: 'ai', ph: 'T', col: '#a98bff', motif: 'A ring of load-bearing pillars, each with a tripwire.',
      g: C(24, 24, 16) + P('M24 8v6M40 24h-6M24 40v-6M8 24h6') + C(24, 24, 4) + P('M33 13l3-3') },
    'automatic-pre-mortem': { n: 'Automatic pre-mortem', fi: 'Automaattinen ennakkoarviointi', fam: 'ai', ph: 'T', col: '#a98bff', motif: 'The pre-mortem fold, triggered by a ledger entry.',
      g: P('M6 34h28a8 8 0 0 0 0-16H14') + P('M18 13l-5 5 5 5') + P('M38 4l-4 8h6l-4 8') },
    'adversarial-engine': { n: 'Adversarial engine', fi: 'Vastaväitemoottori', fam: 'ai', ph: 'T', col: '#ff6b6b', motif: 'Crossed blades inside a gear: a challenger with no ego.',
      g: C(24, 24, 17) + P('M14 14l20 20M34 14 14 34') + P('M24 4v4M24 40v4M4 24h4M40 24h4') },
    'living-scenario-tree': { n: 'Living scenario tree', fi: 'Elävä skenaariopuu', fam: 'ai', ph: 'T', col: '#a98bff', motif: 'Branches whose thickness pulses with live probability.',
      g: P('M24 44V28M24 28 10 10M24 28 38 12M24 34l-14 2') + '<path d="M24 28 38 12" stroke-width="4"/>' + C(10, 10, 2.5, 1) + C(38, 12, 3.5, 1) + C(10, 36, 2, 1) },
    'strategy-coherence-engine': { n: 'Strategy coherence engine', fi: 'Strategian johdonmukaisuusmoottori', fam: 'ai', ph: 'T', col: '#a98bff', motif: 'A graph of documents with one red, contradicting edge.',
      g: C(10, 12, 4) + C(38, 12, 4) + C(10, 36, 4) + C(38, 36, 4) + C(24, 24, 4) + P('M13 15l8 7M35 15l-8 7M13 33l8-7') + '<path d="M27 27l8 7" stroke="#ff5d5d"/>' },
    'double-loop-learning': { n: 'Double-loop learning', fi: 'Kahden silmukan oppiminen', fam: 'ai', ph: 'A', col: '#ff9a3d', motif: 'Two nested loops: fix the action, then fix the assumption.',
      g: P('M24 24c-6-10-18-10-18 0s12 10 18 0c6-10 18-10 18 0s-12 10-18 0') + C(12, 24, 2, 1) + C(36, 24, 2, 1) },
    'domain-expert-gate': { n: 'Domain expert gate', fi: 'Asiantuntijaportti', fam: 'ai', ph: 'A', col: '#ff9a3d', motif: 'A gate that closes itself when an assumption is tagged expert-only.',
      g: P('M6 42V12h36v30M6 20h36') + C(24, 31, 7) + P('M24 27v4l3 2') },
    'autonomy-ladder': { n: 'Autonomy ladder', fi: 'Autonomiaportaat', fam: 'ai', ph: 'A', col: '#ff9a3d', motif: 'Rungs earned one at a time, per decision class, and revocable.',
      g: P('M14 4v40M34 4v40M14 10h20M14 20h20M14 30h20M14 40h20') + C(24, 20, 2.5, 1) },
    'decision-architecture': { n: 'Decision architecture', fi: 'Päätösarkkitehtuuri', fam: 'ai', ph: 'A', col: '#ff9a3d', motif: 'A tiered map of who (or what) decides, and when it escalates.',
      g: '<rect x="18" y="4" width="12" height="8" rx="1"/><rect x="8" y="20" width="12" height="8" rx="1"/><rect x="28" y="20" width="12" height="8" rx="1" stroke-dasharray="3 2"/><rect x="18" y="36" width="12" height="8" rx="1" stroke-dasharray="3 2"/>' + P('M24 12v4M14 16h20M14 16v4M34 16v4M34 28v4M24 32h10M24 32v4') },
    sentinel: { n: 'Sentinel', fi: 'Sentinel', fam: 'ai', ph: 'meta', col: '#7cf2ff', motif: 'A watchtower beam scanning the structured exhaust of every other tool.',
      g: P('M18 44l3-26h6l3 26M16 18h16M20 18V12h8v6') + P('M28 12 44 4M28 14l16 2') },
    'meta-loop': { n: 'Meta-loop', fi: 'Metasilmukka', fam: 'ai', ph: 'meta', col: '#7cf2ff', motif: 'The OTA loop drawn around the toolkit itself.',
      g: C(24, 24, 18) + C(24, 24, 9) + P('M40 18l2 6-6-1') + P('M24 21v6M21 24h6') },
    'data-estate': { n: 'Data estate', fi: 'Datavaranto', fam: 'ai', ph: 'meta', col: '#7cf2ff', motif: 'A stacked vault that compounds: history no rival can buy.',
      g: '<ellipse cx="24" cy="10" rx="16" ry="5"/>' + P('M8 10v28c0 3 7 5 16 5s16-2 16-5V10M8 19c0 3 7 5 16 5s16-2 16-5M8 28c0 3 7 5 16 5s16-2 16-5') },
    'calibrated-trust': { n: 'Calibrated trust', fi: 'Kalibroitu luottamus', fam: 'principle', col: '#7cf2ff', motif: 'A balance between override and abdication.',
      g: P('M24 6v36M10 42h28M8 14h32') + P('M8 14 3 26h10zM40 14l-5 12h10z') },
    // ---- principles & stances
    'paranoid-optimism': { n: 'Paranoid optimism', fi: 'Paranoidi optimismi', fam: 'principle', col: '#f1d27a', motif: 'A sun with a watchful eye: vigilance as the precondition of confidence.',
      g: C(24, 24, 9) + P('M24 4v6M24 38v6M4 24h6M38 24h6M10 10l4 4M34 34l4 4M38 10l-4 4M10 38l4-4') + C(24, 24, 3, 1) },
    agency: { n: 'Agency', fi: 'Toimijuus', fam: 'principle', col: '#f1d27a', motif: 'A hand on the loop: someone owns the last mile.',
      g: C(24, 18, 12) + P('M12 44c0-8 5-12 12-12s12 4 12 12') + P('M24 12v6l4 3') },
    'psychological-safety': { n: 'Psychological safety', fi: 'Psykologinen turvallisuus', fam: 'principle', col: '#5fd38d', motif: 'A shield around an open speech bubble.',
      g: P('M24 4l16 6v12c0 10-7 17-16 22C15 39 8 32 8 22V10z') + P('M16 20h16v8h-9l-4 3v-3h-3z') },
    'ota-constitution': { n: 'OTA constitution', fi: 'OTA-periaatteet', fam: 'principle', col: '#f1d27a', motif: 'A charter of sealed principles: your own operating rules (the book lists seven; its glossary says six).',
      g: P('M10 6h28v36H10z') + C(17, 16, 2, 1) + C(24, 16, 2, 1) + C(31, 16, 2, 1) + C(17, 24, 2, 1) + C(24, 24, 2, 1) + C(31, 24, 2, 1) + P('M15 34h18') }
  };

  // Human → AI evolution (Part IV ch 13)
  const EVOLVES = [
    ['instrument-audit', 'reality-lattice'], ['reality-instrumentation', 'reality-lattice'], ['observation-review', 'signal-brief'],
    ['decision-record', 'decision-ledger'], ['calibration', 'calibration-tracker'], ['wwhtbt', 'assumption-cycle'],
    ['pre-mortem', 'automatic-pre-mortem'], ['red-team', 'adversarial-engine'], ['scenario-mapping', 'living-scenario-tree'],
    ['direction-alignment', 'strategy-coherence-engine'], ['after-action-review', 'double-loop-learning'],
    ['domain-expert-sign-off', 'domain-expert-gate'], ['reversibility', 'autonomy-ladder']
  ];

  function glyph(id, size, color) {
    const c = C_[id]; if (!c) return '';
    const s = size || 24;
    return `<svg class="glyph" width="${s}" height="${s}" viewBox="0 0 48 48" fill="none" stroke="${color || c.col}" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${c.g}</svg>`;
  }
  // badge = glyph inside the family shape
  function badge(id, size) {
    const c = C_[id]; if (!c) return '';
    const s = size || 44, col = c.col, sh = (FAMILIES[c.fam] || {}).shape;
    let frame;
    if (sh === 'hex') frame = '<path d="M24 1 44 12.5v23L24 47 4 35.5v-23z"/>';
    else if (sh === 'diamond') frame = '<path d="M24 1 47 24 24 47 1 24z"/>';
    else if (sh === 'square') frame = '<rect x="3" y="3" width="42" height="42" rx="6"/>';
    else if (sh === 'tri') frame = '<path d="M24 2 46 44H2z"/>';
    else if (sh === 'virus') frame = '<circle cx="24" cy="24" r="19"/><path d="M24 1v4M24 43v4M1 24h4M43 24h4M7.7 7.7l2.8 2.8M37.5 37.5l2.8 2.8M40.3 7.7l-2.8 2.8M10.5 37.5l-2.8 2.8"/>';
    else if (sh === 'ring') frame = '<circle cx="24" cy="24" r="22"/><circle cx="24" cy="24" r="19" stroke-dasharray="2 3"/>';
    else if (sh === 'star') frame = '<path d="M24 2l5 9 10-2-2 10 9 5-9 5 2 10-10-2-5 9-5-9-10 2 2-10-9-5 9-5-2-10 10 2z"/>';
    else frame = '<circle cx="24" cy="24" r="22"/>';
    return `<svg class="badge" width="${s}" height="${s}" viewBox="0 0 48 48" aria-hidden="true">
      <g fill="${col}" fill-opacity=".12" stroke="${col}" stroke-opacity=".85" stroke-width="1.6">${frame}</g>
      <g transform="translate(11.5 11.5) scale(.52)" fill="none" stroke="${col}" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">${c.g}</g></svg>`;
  }

  window.OTA_ID = { FAMILIES, C: C_, EVOLVES, glyph, badge };
})();
