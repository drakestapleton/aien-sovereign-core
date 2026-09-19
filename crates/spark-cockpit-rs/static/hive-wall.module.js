export const HEX_DIRECTIONS = Object.freeze([
  Object.freeze([1, 0]),
  Object.freeze([1, -1]),
  Object.freeze([0, -1]),
  Object.freeze([-1, 0]),
  Object.freeze([-1, 1]),
  Object.freeze([0, 1]),
]);

export const HEX_DIRECTION_LABELS = Object.freeze([
  "east",
  "northeast",
  "northwest",
  "west",
  "southwest",
  "southeast",
]);

const PLACEMENT_INTENTS = new Set(["independent", "join", "branch", "meet"]);
const CONVERSATION_HUES = Object.freeze([34, 52, 76, 105, 137, 168, 196, 218, 244, 271, 298, 326, 8]);

export const WALL_LIMITS = Object.freeze({
  maxRadius: 512,
  bodyCharacters: 600,
  chunkSize: 16,
  // The floor is only the hard stop. The usable floor is computed per wall by
  // fitZoom() so "zoom all the way out" always means "the whole Hive fits",
  // never an arbitrary percentage that leaves combs off screen.
  minZoom: 0.08,
  maxZoom: 1.8,
});

// Reading text stops being possible long before the hexes stop being useful
// shapes, so the Wall drops detail in tiers instead of shrinking unreadable
// paragraphs. Each tier is a CSS contract, not a re-render.
export const DETAIL_TIERS = Object.freeze([
  Object.freeze({ id: "full", minZoom: 0.62 }),
  Object.freeze({ id: "compact", minZoom: 0.34 }),
  Object.freeze({ id: "dot", minZoom: 0 }),
]);

const SQRT_3 = Math.sqrt(3);
const DEFAULT_HEX_RADIUS = 88;
const WORLD_MARGIN = 170;
// Cap the chunk fan-out one viewport can request. Zoomed all the way out a
// large wall would otherwise ask for every chunk at once; the nearest chunks
// to the camera load first and the rest arrive as the viewer moves.
const MAX_VISIBLE_CHUNKS = 24;
const ZOOM_STEP = 1.28;
const PAN_KEY_STEP = 140;
const DRAFT_KEY = "hive_wall_draft_v1";
const EDIT_DRAFT_KEY = "hive_wall_edit_draft_v1";
const DRAFT_HANDOFF_KEY = "hive_wall_draft_handoff_v1";
const DRAFT_HANDOFF_TTL_MS = 20 * 60 * 1000;
const SEEN_KEY = "hive_wall_seen_v1";
const MODE_KEY = "hive_wall_mode_v1";
const MAX_CACHED_CHUNKS = 32;
const MAX_CACHED_CELLS = 5_000;
const MAX_LIST_RECONCILE_EVENTS = 256;

export function cellKey(q, r) {
  return `${Number(q)}:${Number(r)}`;
}

export function hexDistance(q, r, otherQ = 0, otherR = 0) {
  const dq = Number(q) - Number(otherQ);
  const dr = Number(r) - Number(otherR);
  return Math.max(Math.abs(dq), Math.abs(dr), Math.abs(dq + dr));
}

export function axialToPixel(q, r, radius = DEFAULT_HEX_RADIUS) {
  return {
    x: radius * SQRT_3 * (Number(q) + Number(r) / 2),
    y: radius * 1.5 * Number(r),
  };
}

export function roundAxial(q, r) {
  let cubeX = Number(q);
  let cubeZ = Number(r);
  let cubeY = -cubeX - cubeZ;
  let roundedX = Math.round(cubeX);
  let roundedY = Math.round(cubeY);
  let roundedZ = Math.round(cubeZ);
  const dx = Math.abs(roundedX - cubeX);
  const dy = Math.abs(roundedY - cubeY);
  const dz = Math.abs(roundedZ - cubeZ);
  if (dx > dy && dx > dz) roundedX = -roundedY - roundedZ;
  else if (dy > dz) roundedY = -roundedX - roundedZ;
  else roundedZ = -roundedX - roundedY;
  return { q: roundedX, r: roundedZ };
}

export function pixelToAxial(x, y, radius = DEFAULT_HEX_RADIUS) {
  const q = (SQRT_3 / 3 * Number(x) - Number(y) / 3) / radius;
  const r = (2 / 3 * Number(y)) / radius;
  return roundAxial(q, r);
}

export function neighbors(q, r) {
  return HEX_DIRECTIONS.map(([dq, dr]) => ({ q: Number(q) + dq, r: Number(r) + dr }));
}

export function chunkFor(q, r, size = WALL_LIMITS.chunkSize) {
  const qMin = Math.floor(Number(q) / size) * size;
  const rMin = Math.floor(Number(r) / size) * size;
  return Object.freeze({
    key: `${qMin}:${rMin}`,
    qMin,
    qMax: qMin + size - 1,
    rMin,
    rMax: rMin + size - 1,
  });
}

export function conversationChoices(cells, q, r, conversations = null) {
  return adjacentConversationGroups(cells, q, r, conversations)
    .map((group) => group.anchors[0]?.cell)
    .filter(Boolean);
}

export function adjacentConversationGroups(cells, q, r, conversations = null) {
  const groups = new Map();
  neighbors(q, r).forEach((coordinate, direction) => {
    const cell = cells instanceof Map ? cells.get(cellKey(coordinate.q, coordinate.r)) : null;
    if (!cell?.conversationId || (cell.status && cell.status !== "visible")) return;
    if (conversations instanceof Map) {
      const conversation = conversations.get(cell.conversationId);
      if (!conversation || conversation.status !== "open") return;
    }
    let group = groups.get(cell.conversationId);
    if (!group) {
      group = { conversationId: cell.conversationId, anchors: [] };
      groups.set(cell.conversationId, group);
    }
    group.anchors.push({ cell, direction, directionLabel: HEX_DIRECTION_LABELS[direction] });
  });
  return [...groups.values()];
}

export function canonicalAnchorIds(anchorCellIds) {
  return [...new Set((anchorCellIds || []).filter((value) => typeof value === "string" && value))].sort();
}

export function reconcileAccessibleListWindow(existing, incoming, conversationId = null) {
  const byId = new Map();
  for (const cell of [...(existing || []), ...(incoming || [])]) {
    if (cell && cell.id) byId.set(cell.id, cell);
  }
  return [...byId.values()].sort(conversationId
    ? (left, right) => left.seq - right.seq
    : (left, right) => right.seq - left.seq);
}

export function accessibleListRefreshFrontier({
  reset,
  priorCells,
  incomingCells,
  priorBeforeSeq,
  priorHasMore,
  nextBeforeSeq,
  nextHasMore,
}) {
  const priorIds = new Set((priorCells || []).map((cell) => cell?.id).filter(Boolean));
  const overlapsLoaded = Boolean(reset && priorIds.size
    && (incomingCells || []).some((cell) => priorIds.has(cell?.id)));
  return overlapsLoaded
    ? { beforeSeq: priorBeforeSeq, hasMore: priorHasMore }
    : { beforeSeq: nextBeforeSeq, hasMore: nextHasMore };
}

function clamp(value, minimum, maximum) {
  return Math.min(maximum, Math.max(minimum, value));
}

function escapeSelectorValue(value) {
  if (globalThis.CSS && typeof globalThis.CSS.escape === "function") return globalThis.CSS.escape(String(value));
  return String(value).replace(/["\\]/g, "\\$&");
}

function setTextIfChanged(node, value) {
  if (!node) return;
  const next = String(value == null ? "" : value);
  if (node.textContent !== next) node.textContent = next;
}

function validCoordinate(q, r, radius = WALL_LIMITS.maxRadius) {
  return Number.isInteger(q) && Number.isInteger(r) && hexDistance(q, r) <= radius;
}

function safeJson(storage, key, fallback) {
  try {
    const parsed = JSON.parse(storage.getItem(key));
    return parsed == null ? fallback : parsed;
  } catch {
    return fallback;
  }
}

function safeSet(storage, key, value) {
  try {
    const encoded = JSON.stringify(value);
    storage.setItem(key, encoded);
    return storage.getItem(key) === encoded && storage.__hivePersistent !== false;
  } catch {
    // Private browsing and full storage must not break the wall.
    return false;
  }
}

function memoryStorage() {
  const values = new Map();
  return {
    __hivePersistent: false,
    getItem(key) { return values.has(key) ? values.get(key) : null; },
    setItem(key, value) { values.set(key, String(value)); },
    removeItem(key) { values.delete(key); },
  };
}

function availableStorage(provided) {
  if (provided) return provided;
  try {
    const storage = globalThis.localStorage;
    const probe = "__hive_wall_storage_probe__";
    storage.setItem(probe, "1");
    storage.removeItem(probe);
    return storage;
  } catch {
    return memoryStorage();
  }
}

function availableSessionStorage(provided) {
  if (provided) return provided;
  try {
    const storage = globalThis.sessionStorage;
    const probe = "__hive_wall_session_probe__";
    storage.setItem(probe, "1");
    storage.removeItem(probe);
    return storage;
  } catch {
    return memoryStorage();
  }
}

function normalizedPlacementDraft(source, resetRequestId = false) {
  if (!source || !validCoordinate(Number(source.q), Number(source.r))) return null;
  const body = typeof source.body === "string"
    ? [...source.body].slice(0, WALL_LIMITS.bodyCharacters).join("")
    : "";
  const inferredIntent = source.intent && PLACEMENT_INTENTS.has(source.intent)
    ? source.intent
    : source.conversationId ? "join" : "independent";
  return {
    q: Number(source.q),
    r: Number(source.r),
    body,
    requestId: resetRequestId ? null : (typeof source.requestId === "string" ? source.requestId : null),
    conversationId: typeof source.conversationId === "string" ? source.conversationId : null,
    replyToId: typeof source.replyToId === "string" ? source.replyToId : null,
    intent: inferredIntent,
    anchorCellIds: canonicalAnchorIds(source.anchorCellIds),
    choiceMade: source.choiceMade !== false,
    choiceExplicit: source.choiceExplicit === true,
    requiresConversationChoice: source.requiresConversationChoice === true,
    mode: typeof source.mode === "string" ? source.mode : null,
    updatedAt: Number(source.updatedAt) || Date.now(),
  };
}

function randomRequestId() {
  if (globalThis.crypto && typeof globalThis.crypto.randomUUID === "function") {
    return globalThis.crypto.randomUUID();
  }
  const bytes = new Uint8Array(16);
  if (globalThis.crypto && typeof globalThis.crypto.getRandomValues === "function") {
    globalThis.crypto.getRandomValues(bytes);
  } else {
    for (let i = 0; i < bytes.length; i += 1) bytes[i] = Math.floor(Math.random() * 256);
  }
  bytes[6] = (bytes[6] & 15) | 64;
  bytes[8] = (bytes[8] & 63) | 128;
  const hex = [...bytes].map((value) => value.toString(16).padStart(2, "0"));
  return `${hex.slice(0, 4).join("")}-${hex.slice(4, 6).join("")}-${hex.slice(6, 8).join("")}-${hex.slice(8, 10).join("")}-${hex.slice(10).join("")}`;
}

function hueFor(value) {
  let hash = 2166136261;
  for (const character of String(value || "hive")) {
    hash ^= character.charCodeAt(0);
    hash = Math.imul(hash, 16777619);
  }
  return CONVERSATION_HUES[Math.abs(hash) % CONVERSATION_HUES.length];
}

function patternFor(value) {
  let hash = 0;
  for (const character of String(value || "hive")) hash = Math.imul(hash ^ character.charCodeAt(0), 16777619);
  return Math.abs(hash) % 4;
}

function timeLabel(timestamp) {
  const value = Number(timestamp);
  if (!Number.isFinite(value)) return "";
  const elapsed = Date.now() - value;
  if (elapsed < 60_000) return "just now";
  if (elapsed < 3_600_000) return `${Math.max(1, Math.floor(elapsed / 60_000))}m`;
  if (elapsed < 86_400_000) return `${Math.floor(elapsed / 3_600_000)}h`;
  return new Date(value).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

function statusText(cell) {
  if (cell.viewerHidden) return "Hidden after your private report";
  if (cell.status === "author_removed") return "Message removed by its author";
  if (cell.status === "quarantined") return "Message under review";
  if (cell.status === "moderator_removed") return "Message removed by the Hive watch";
  return cell.body || "";
}

function isEditableField(target) {
  return target instanceof Element && Boolean(target.closest("input, textarea, select, [contenteditable='true']"));
}

function staticMarkup() {
  return `
    <section class="honey-wall" aria-labelledby="honeyWallTitle">
      <h1 id="honeyWallTitle" class="sr-only">The Hive conversation wall</h1>
      <div class="honey-toolbar" aria-label="Hive wall controls">
        <div class="honey-toolbar-group honey-toolbar-main">
          <button type="button" class="honey-tool honey-tool-primary" data-wall-action="add">Add a comb</button>
          <button type="button" class="honey-tool honey-tool-mode" data-wall-action="mode" aria-pressed="false">Read as list</button>
          <button type="button" class="honey-tool honey-resume-tool" data-wall-action="resume" hidden>Resume draft</button>
          <button type="button" class="honey-tool" data-wall-action="new">Find new <span data-wall-new-count hidden></span></button>
          <button type="button" class="honey-tool" data-wall-action="mine">My combs</button>
          <button type="button" class="honey-tool honey-watch-tool" data-wall-action="watch" hidden>Hive watch</button>
        </div>
        <div class="honey-toolbar-group honey-toolbar-view" aria-label="Move and zoom the wall">
          <button type="button" class="honey-tool" data-wall-action="center">Center</button>
          <button type="button" class="honey-tool" data-wall-action="fit">See it all</button>
          <button type="button" class="honey-tool" data-wall-action="zoom-out" aria-label="Zoom out">−</button>
          <output class="honey-zoom" data-wall-zoom aria-label="Wall zoom">100%</output>
          <button type="button" class="honey-tool" data-wall-action="zoom-in" aria-label="Zoom in">+</button>
        </div>
      </div>
      <header class="honey-welcome">
        <p class="honey-eyebrow">One wall · countless conversations</p>
        <h2>The page is the Hive.</h2>
        <p>Conversations grow beside each other instead of being sorted into a feed.</p>
        <details class="honey-start-guide" data-wall-onboarding open>
          <summary>Start here · three simple steps</summary>
          <ol>
            <li><strong>Pick a comb.</strong><span>Tap a message to read it, or choose Add a comb to begin.</span></li>
            <li><strong>Choose a direction.</strong><span>Continue nearby, start a branch, or let conversations meet.</span></li>
            <li><strong>Write and place it.</strong><span>Your message stays attached to the path you chose.</span></li>
          </ol>
          <p>Drag to move around. Pinch or use the zoom controls to change the view. Prefer a straight reading order? Choose <strong>Read as list</strong>.</p>
        </details>
      </header>
      <div class="honey-summary" aria-hidden="true">
        <span class="honey-summary-mark">THE HIVE</span>
        <span data-wall-summary>Loading the wall…</span>
      </div>
      <p class="sr-only" data-wall-state-status role="status" aria-live="polite">Loading the shared Wall.</p>
      <p id="honeyWallKeyboardHelp" class="sr-only">Use arrow keys to explore. For all six directions use E northeast, D east, C southeast, Z southwest, A west, and Q northwest. Hold Shift with an arrow key to glide the view without moving your place. Press plus to zoom in, minus to zoom out, and 0 to pull back until the whole Hive fits. Press Enter to open or write. Escape cancels choosing a branch.</p>
      <div class="honey-stage" data-wall-stage tabindex="0" role="application" aria-label="Honeycomb message wall" aria-describedby="honeyWallKeyboardHelp">
        <div class="honey-plane" data-wall-plane>
          <div class="honey-connections" data-wall-connections aria-hidden="true"></div>
          <div class="honey-cells" data-wall-cells></div>
          <div class="honey-growth-slots" data-wall-growth-slots aria-label="Open places around this comb"></div>
          <div class="honey-target" data-wall-target aria-hidden="true"><span>+</span></div>
        </div>
      </div>
      <div class="honey-growth-tray" data-wall-growth-tray role="status" hidden>
        <span data-wall-growth-copy></span>
        <button type="button" class="honey-tool" data-wall-action="nearest-growth" hidden>Find nearest open edge</button>
        <button type="button" class="honey-tool" data-wall-action="cancel-growth">Cancel</button>
      </div>
      <section class="honey-list" data-wall-list hidden tabindex="-1" aria-labelledby="honeyListTitle">
        <div class="honey-list-head">
          <div><p class="honey-eyebrow">Accessible wall view</p><h2 id="honeyListTitle">Every comb with its connections</h2></div>
          <div class="honey-list-actions">
            <button type="button" class="honey-tool" data-wall-action="list-all" data-wall-list-focus="list-all" hidden>All messages</button>
            <button type="button" class="honey-tool" data-wall-action="load-more" data-wall-list-focus="load-more">Load more</button>
          </div>
        </div>
        <div data-wall-list-items></div>
      </section>
      <div class="honey-edge-note" data-wall-edge-note>Click any open place in the honeycomb to leave a message.</div>
      <div class="honey-live sr-only" data-wall-live aria-live="polite" aria-atomic="true"></div>

      <dialog class="honey-sheet honey-compose" data-wall-compose aria-labelledby="honeyComposeTitle">
        <form method="dialog" data-wall-compose-form>
          <div class="honey-sheet-grip" aria-hidden="true"></div>
          <div class="honey-sheet-head">
            <div><p class="honey-eyebrow" data-wall-compose-kicker>Open comb</p><h2 id="honeyComposeTitle" data-wall-compose-title>Leave a message</h2></div>
            <button type="button" class="honey-sheet-close" data-wall-close="compose" aria-label="Close composer">×</button>
          </div>
          <p class="honey-compose-place" data-wall-compose-place></p>
          <fieldset class="honey-conversation-choices" data-wall-conversation-choices hidden>
            <legend data-wall-conversation-legend>How should this comb connect?</legend>
            <div data-wall-conversation-options></div>
          </fieldset>
          <label class="honey-compose-label" for="honeyComposeBody">Your message</label>
          <textarea id="honeyComposeBody" data-wall-compose-body rows="5" maxlength="600" required aria-describedby="honeyComposePrivacy honeyComposeCount" aria-errormessage="honeyComposeError" placeholder="Leave something worth finding here…"></textarea>
          <div class="honey-compose-meta" id="honeyComposePrivacy">
            <span>Public to everyone who visits the Hive.</span>
            <output id="honeyComposeCount" data-wall-compose-count>0 / 600</output>
          </div>
          <p class="honey-compose-error" id="honeyComposeError" data-wall-compose-error role="alert" hidden></p>
          <div class="honey-sheet-actions">
            <button type="button" class="honey-tool" data-wall-close="compose" data-wall-compose-cancel>Keep as draft</button>
            <button type="button" class="honey-tool" data-wall-compose-suggestion hidden>Use highlighted comb</button>
            <button type="submit" class="honey-tool honey-tool-primary" data-wall-compose-submit>Place this comb</button>
          </div>
        </form>
      </dialog>

      <dialog class="honey-sheet honey-reader" data-wall-reader aria-labelledby="honeyReaderTitle">
        <div class="honey-sheet-grip" aria-hidden="true"></div>
        <div class="honey-sheet-head">
          <div><p class="honey-eyebrow" data-wall-reader-kicker>Conversation comb</p><h2 id="honeyReaderTitle" data-wall-reader-author></h2></div>
          <button type="button" class="honey-sheet-close" data-wall-close="reader" aria-label="Close message">×</button>
        </div>
        <p class="honey-reader-meta" data-wall-reader-meta></p>
        <div class="honey-reader-body" data-wall-reader-body></div>
        <div class="honey-participants" data-wall-reader-participants></div>
        <div class="honey-relations" data-wall-reader-relations></div>
        <div class="honey-sheet-actions honey-reader-actions">
          <button type="button" class="honey-tool honey-tool-primary" data-wall-reader-action="reply">Reply from this comb</button>
          <button type="button" class="honey-tool" data-wall-reader-action="branch">Start a new branch</button>
          <button type="button" class="honey-tool" data-wall-reader-action="show">Show on wall</button>
          <button type="button" class="honey-tool" data-wall-reader-action="edit" hidden>Edit</button>
          <button type="button" class="honey-tool" data-wall-reader-action="remove" hidden>Remove</button>
          <button type="button" class="honey-tool honey-report" data-wall-reader-action="report">Report</button>
          <button type="button" class="honey-tool honey-watch-open" data-wall-reader-action="watch-open" hidden>Open urgent review</button>
        </div>
        <form class="honey-report-form" data-wall-report-form hidden>
          <label for="honeyReportReason">Why should the Hive watch look at this?</label>
          <select id="honeyReportReason" data-wall-report-reason required>
            <option value="" selected disabled>Choose a reason</option>
            <option value="personal_attack">Attack aimed at a person</option>
            <option value="illegal">Illegal content</option>
            <option value="spam">Spam or flooding</option>
            <option value="privacy">Private information</option>
          </select>
          <div class="honey-sheet-actions">
            <button type="button" class="honey-tool" data-wall-reader-action="cancel-report">Cancel</button>
            <button type="submit" class="honey-tool honey-tool-primary">Send private report</button>
          </div>
        </form>
        <form class="honey-operator-form" data-wall-operator-form hidden>
          <label for="honeyOperatorReason">Why does this comb need urgent human review?</label>
          <textarea id="honeyOperatorReason" data-wall-operator-reason maxlength="500" rows="3" required placeholder="Record the evidence-based reason for opening this case."></textarea>
          <div class="honey-sheet-actions">
            <button type="button" class="honey-tool" data-wall-reader-action="cancel-watch-open">Cancel</button>
            <button type="submit" class="honey-tool honey-tool-primary">Open private case</button>
          </div>
        </form>
      </dialog>

      <dialog class="honey-sheet honey-watch" data-wall-watch aria-labelledby="honeyWatchTitle">
        <div class="honey-sheet-grip" aria-hidden="true"></div>
        <div class="honey-watch-inner">
          <div class="honey-sheet-head">
            <div><p class="honey-eyebrow">Allowlisted human review</p><h2 id="honeyWatchTitle">Hive watch</h2></div>
            <button type="button" class="honey-sheet-close" data-wall-close="watch" aria-label="Close Hive watch">×</button>
          </div>
          <p class="honey-watch-status" data-wall-watch-status role="status" aria-live="polite"></p>
          <div class="honey-watch-queue" data-wall-watch-queue></div>
          <button type="button" class="honey-tool honey-watch-more" data-wall-watch-more data-wall-watch-focus="more" hidden>Load more cases</button>
          <section class="honey-watch-detail" id="honeyWatchDetail" data-wall-watch-detail hidden aria-labelledby="honeyWatchCaseTitle">
            <h3 id="honeyWatchCaseTitle" tabindex="-1">Review case</h3>
            <p data-wall-watch-meta></p>
            <blockquote data-wall-watch-evidence></blockquote>
            <p data-wall-watch-current></p>
            <div data-wall-watch-reports></div>
            <div data-wall-watch-events></div>
            <label for="honeyWatchReason">Decision note</label>
            <textarea id="honeyWatchReason" data-wall-watch-reason maxlength="500" rows="3" placeholder="State the evidence-based reason for this action."></textarea>
            <div class="honey-sheet-actions" data-wall-watch-actions></div>
          </section>
        </div>
      </dialog>
    </section>`;
}

export class HiveWallController {
  constructor(options = {}) {
    if (!options.root) throw new Error("HiveWall needs a root element.");
    if (!options.backend) throw new Error("HiveWall needs the shared backend.");
    this.root = options.root;
    this.backend = options.backend;
    this.getUser = typeof options.getUser === "function" ? options.getUser : () => null;
    this.requestIdentity = typeof options.requestIdentity === "function" ? options.requestIdentity : () => {};
    this.toast = typeof options.toast === "function" ? options.toast : () => {};
    this.onCellLink = typeof options.onCellLink === "function" ? options.onCellLink : null;
    this.onPlaced = typeof options.onPlaced === "function" ? options.onPlaced : null;
    this.storage = availableStorage(options.storage);
    this.handoffStorage = availableSessionStorage(options.handoffStorage);
    this.hexRadius = Number(options.hexRadius) || DEFAULT_HEX_RADIUS;
    this.cells = new Map();
    this.cellsById = new Map();
    this.conversations = new Map();
    this.nodes = new Map();
    this.connectionNodes = new Map();
    this.loadedChunks = new Set();
    this.chunkAccess = new Map();
    this.cellChunks = new Map();
    this.dirtyChunks = new Set();
    this.inflightChunks = new Map();
    this.visibleChunkKeys = new Set();
    this.pendingEvents = new Map();
    this.retryEvents = new Map();
    // One obligation per changed comb in the retained accessible-list window.
    // These retry with bounded backoff until an authoritative cell read
    // succeeds. Route/identity changes discard the retained list itself, so no
    // stale hidden window can later reappear under a different context.
    this.listReconcileEvents = new Map();
    this.consumingEvents = new Map();
    this.realtimeDrainQueue = new Map();
    this.realtimeDrainPromise = null;
    this.realtimeDrainDirty = false;
    this.seenRealtimeEvents = new Set();
    this.forceRefreshOnActivate = false;
    this.publicCacheGeneration = 0;
    this.wallWindowRevision = 0;
    this.bounds = {
      revision: 0,
      viewerRevision: 0,
      radius: 5,
      messageCount: 0,
      readOnly: true,
      updatedAt: 0,
    };
    this.mode = this.storage && this.storage.getItem(MODE_KEY) === "list" ? "list" : "wall";
    this.zoom = 1;
    this.cameraX = 0;
    // The Wall is a map the viewer flies over, so the camera owns both axes.
    // Vertical movement used to be document scroll, which made "zoom out and
    // look around" impossible once the plane was shorter than the page.
    this.cameraY = 0;
    this.world = { halfWidth: 0, halfHeight: 0, width: 0, height: 0, originX: 0, originY: 0 };
    this.detailTier = null;
    this.pinch = null;
    this.selected = { q: 0, r: 0 };
    this.selectedCellKey = null;
    this.readerCell = null;
    this.reportRequest = null;
    this.operatorCaseRequest = null;
    this.watchActionRequest = null;
    this.composeState = null;
    this.growthIntent = null;
    this.placementIntentGeneration = 0;
    this.focusGeneration = 0;
    this.composeGeneration = 0;
    this.readerGeneration = 0;
    this.watchGeneration = 0;
    this.watchQueueRequestGeneration = 0;
    this.watchCaseRequestGeneration = 0;
    this.watchActionGeneration = 0;
    this.operatorProbeGeneration = 0;
    this.scopeRequestGeneration = 0;
    this.composeBusy = false;
    this.readerBusy = false;
    this.operator = null;
    this.watchCase = null;
    this.watchLoadingGeneration = 0;
    this.watchBusy = false;
    this.watchCases = [];
    this.watchCursor = null;
    this.watchHasMore = false;
    this.watchPollTimer = null;
    this.active = false;
    this.mounted = false;
    this.unsubscribe = null;
    this.realtimeTeardown = Promise.resolve();
    this.pollTimer = null;
    this.eventTimer = null;
    this.eventRetryTimer = null;
    this.listReconcileTimer = null;
    this.renderFrame = null;
    this.glideTimer = null;
    this.momentumFrame = null;
    this.viewportRefreshPromise = null;
    this.viewportRefreshDirty = false;
    this.viewportRefreshForce = false;
    this.focusFrame = null;
    this.growthFitFrame = null;
    this.announceFrame = null;
    this.eventAbort = null;
    this.dialogOpeners = new Map();
    this.dialogFocusAnchors = new Map();
    this.pointer = null;
    this.activePointers = new Map();
    this.suppressCellClickUntil = 0;
    this.suppressBackdropClick = null;
    this.listCells = [];
    this.listBeforeSeq = null;
    this.listHasMore = true;
    this.listConversationId = null;
    this.listWindowRevision = 0;
    this.listRequestGeneration = 0;
    this.boundsRequestGeneration = 0;
    this.boundsSuccessGeneration = 0;
    this.lastGoodAt = 0;
    this.hasActivated = false;
    this.activationGeneration = 0;
    this.realtimeGeneration = 0;
    this.transportState = "polling";
    this.dataStale = false;
    this.storageScope = null;
    this.identityGeneration = 0;
    this.seen = new Set();
    this.draft = null;
    this.editDraft = null;
    this.draftPersistenceFailed = false;
    this.editDraftPersistenceFailed = false;
    this.draftFromHandoff = false;
    this.quarantinedDrafts = new Map();
    this.orphanedPrivateDraft = null;
    this.viewportWidth = window.innerWidth;
    this.boundViewport = (event) => {
      this.updateVisualViewport();
      const nextWidth = window.innerWidth;
      if (this.growthIntent) {
        this.viewportWidth = nextWidth;
        if (this.growthFitFrame) cancelAnimationFrame(this.growthFitFrame);
        this.growthFitFrame = requestAnimationFrame(() => {
          this.growthFitFrame = null;
          this.fitGrowthIntent();
        });
        this.scheduleViewport();
        return;
      }
      // Page scroll moves the stage through the document but no longer changes
      // which part of the wall the camera sees, so it needs no camera work and
      // must not schedule any: this listener also fires from scrolls the wall
      // itself performs.
      if (event?.type === "scroll") return;
      this.viewportWidth = nextWidth;
      // A rotation or keyboard dismissal changes how much world fits, which
      // changes the usable zoom floor. Re-settle the camera before repainting
      // so a viewer who was zoomed all the way out stays zoomed all the way
      // out instead of being left below the new floor.
      this.setZoom(this.zoom, null, { announce: false });
    };
    this.boundVisualViewport = () => this.boundViewport({ type: "visualviewport" });
    this.boundVisibility = () => {
      if (document.visibilityState === "visible" && this.active) {
        if (this.mode === "list") this.refreshList(true);
        else this.refreshVisible(true);
        this.wakeListReconciliation();
      }
    };
    this.boundOnline = () => {
      this.setConnectionState(navigator.onLine ? (this.dataStale ? "stale" : this.transportState) : "offline");
      if (navigator.onLine) this.wakeListReconciliation();
    };
  }

  mount() {
    if (this.mounted) return;
    this.eventAbort = new AbortController();
    this.root.innerHTML = staticMarkup();
    this.wall = this.root.querySelector(".honey-wall");
    this.stage = this.root.querySelector("[data-wall-stage]");
    this.plane = this.root.querySelector("[data-wall-plane]");
    this.cellLayer = this.root.querySelector("[data-wall-cells]");
    this.connectionLayer = this.root.querySelector("[data-wall-connections]");
    this.growthSlots = this.root.querySelector("[data-wall-growth-slots]");
    this.growthTray = this.root.querySelector("[data-wall-growth-tray]");
    this.target = this.root.querySelector("[data-wall-target]");
    this.live = this.root.querySelector("[data-wall-live]");
    this.summary = this.root.querySelector("[data-wall-summary]");
    this.newCount = this.root.querySelector("[data-wall-new-count]");
    this.zoomOutput = this.root.querySelector("[data-wall-zoom]");
    this.list = this.root.querySelector("[data-wall-list]");
    this.listItems = this.root.querySelector("[data-wall-list-items]");
    this.compose = this.root.querySelector("[data-wall-compose]");
    this.composeBody = this.root.querySelector("[data-wall-compose-body]");
    this.composeError = this.root.querySelector("[data-wall-compose-error]");
    this.reader = this.root.querySelector("[data-wall-reader]");
    this.watch = this.root.querySelector("[data-wall-watch]");
    this.watchQueue = this.root.querySelector("[data-wall-watch-queue]");
    this.watchDetail = this.root.querySelector("[data-wall-watch-detail]");
    this.watchReason = this.root.querySelector("[data-wall-watch-reason]");
    this.bindEvents();
    this.applyMode();
    this.updateWorldSize();
    this.updateTarget();
    this.mounted = true;
  }

  bindEvents() {
    const listener = { signal: this.eventAbort.signal };
    this.root.addEventListener("click", (event) => this.handleClick(event), listener);
    this.root.addEventListener("keydown", (event) => {
      if (event.key === "Escape" && this.growthIntent && !event.defaultPrevented) {
        event.preventDefault();
        this.cancelGrowthIntent();
      }
    }, listener);
    this.stage.addEventListener("keydown", (event) => this.handleStageKey(event), listener);
    this.stage.addEventListener("pointerdown", (event) => this.handlePointerDown(event), listener);
    this.stage.addEventListener("pointermove", (event) => this.handlePointerMove(event), listener);
    this.stage.addEventListener("pointerup", (event) => this.handlePointerUp(event), listener);
    this.stage.addEventListener("pointercancel", (event) => this.cancelPointerGesture(event), listener);
    this.stage.addEventListener("wheel", (event) => this.handleWheel(event), { ...listener, passive: false });
    this.stage.addEventListener("dblclick", (event) => this.handleDoubleClick(event), listener);
    this.stage.addEventListener("focusin", (event) => this.handleStageFocus(event), listener);
    if (typeof globalThis.ResizeObserver === "function") {
      this.trayObserver = new globalThis.ResizeObserver(() => {
        const height = this.growthTray?.getBoundingClientRect?.().height || 0;
        // Only a real height change matters, and only while a picker is open.
        // Refitting on equal heights would spin against the viewport listener.
        if (!this.growthIntent || Math.abs(height - (this.lastGrowthTrayHeight || 0)) < 1) return;
        this.lastGrowthTrayHeight = height;
        this.scheduleGrowthFit();
      });
      this.trayObserver.observe(this.growthTray);
    }
    this.root.querySelector("[data-wall-compose-form]").addEventListener("submit", (event) => {
      event.preventDefault();
      void this.submitCompose();
    }, listener);
    this.composeBody.addEventListener("input", () => {
      this.root.querySelector("[data-wall-compose-count]").textContent = `${[...this.composeBody.value].length} / 600`;
      if (this.composeState) this.composeState.requestId = null;
      const persisted = this.composeState?.editing ? this.saveEditDraft() : this.saveDraft();
      if (persisted !== false) this.hideComposeError();
    }, listener);
    this.composeBody.addEventListener("focus", () => this.updateVisualViewport(), listener);
    this.root.querySelector("[data-wall-report-form]").addEventListener("submit", (event) => {
      event.preventDefault();
      void this.submitReport();
    }, listener);
    this.root.querySelector("[data-wall-report-reason]").addEventListener("change", () => {
      this.reportRequest = null;
    }, listener);
    this.root.querySelector("[data-wall-operator-reason]").addEventListener("input", () => {
      this.operatorCaseRequest = null;
    }, listener);
    this.root.querySelector("[data-wall-watch-reason]").addEventListener("input", () => {
      this.watchActionRequest = null;
    }, listener);
    this.root.querySelector("[data-wall-operator-form]").addEventListener("submit", (event) => {
      event.preventDefault();
      void this.submitOperatorCase();
    }, listener);
    this.root.querySelector("[data-wall-compose-suggestion]").addEventListener("click", () => this.acceptComposeSuggestion(), listener);
    for (const dialog of [this.compose, this.reader, this.watch]) {
      dialog.addEventListener("cancel", (event) => {
        event.preventDefault();
        this.closeDialog(dialog);
      }, listener);
      dialog.addEventListener("click", (event) => {
        if (event.target !== dialog) return;
        const suppressed = this.suppressBackdropClick;
        if (suppressed?.dialog === dialog && Date.now() <= suppressed.until) {
          this.suppressBackdropClick = null;
          event.preventDefault();
          event.stopPropagation();
          return;
        }
        if (suppressed?.dialog === dialog) this.suppressBackdropClick = null;
        this.closeDialog(dialog);
      }, listener);
    }
  }

  async activate(options = {}) {
    this.mount();
    if (this.active) {
      if (options.cellId) await this.focusCell(options.cellId);
      return;
    }
    const generation = ++this.activationGeneration;
    this.active = true;
    this.wall.classList.add("is-active");
    await this.syncStorageScope(false);
    if (!this.isActivationCurrent(generation)) return;
    window.addEventListener("resize", this.boundViewport, { passive: true });
    window.addEventListener("scroll", this.boundViewport, { passive: true });
    window.visualViewport?.addEventListener?.("resize", this.boundVisualViewport, { passive: true });
    window.visualViewport?.addEventListener?.("scroll", this.boundVisualViewport, { passive: true });
    window.addEventListener("online", this.boundOnline);
    window.addEventListener("offline", this.boundOnline);
    document.addEventListener("visibilitychange", this.boundVisibility);
    this.updateVisualViewport();
    try {
      await this.startRealtime();
    } catch {
      this.unsubscribe = null;
      this.transportState = "polling";
      this.setConnectionState(navigator.onLine ? "polling" : "offline");
    }
    if (!this.isActivationCurrent(generation)) return;
    await this.refreshBounds();
    if (!this.isActivationCurrent(generation)) return;
    const firstActivation = !this.hasActivated && !options.cellId;
    if (firstActivation) this.revealCoordinate(0, 0, "auto");
    this.hasActivated = true;
    const forceVisibleRefresh = this.forceRefreshOnActivate;
    this.forceRefreshOnActivate = false;
    await this.refreshVisible(forceVisibleRefresh);
    if (!this.isActivationCurrent(generation)) return;
    if (firstActivation && this.mode === "wall" && this.viewportWidth <= 760) {
      this.frameInitialPhoneView();
    }
    void this.probeOperator();
    if (this.mode === "list") {
      await this.refreshList(true);
      if (!this.isActivationCurrent(generation)) return;
      this.wakeListReconciliation();
    }
    if (this.pollTimer) window.clearInterval(this.pollTimer);
    this.pollTimer = window.setInterval(() => {
      if (this.active && document.visibilityState === "visible") {
        if (this.mode === "list") void this.refreshList(true);
        else void this.refreshVisible(true);
        if (this.operator) void this.probeOperator();
      }
    }, 45_000);
    if (options.cellId) {
      await this.focusCell(options.cellId);
      if (!this.isActivationCurrent(generation)) return;
    }
    if (this.draft && validCoordinate(this.draft.q, this.draft.r, WALL_LIMITS.maxRadius)) {
      this.announce("Your unfinished wall message is still here.");
    }
    this.updateResumeDraftAction();
  }

  deactivate() {
    this.activationGeneration += 1;
    this.realtimeGeneration += 1;
    this.scopeRequestGeneration += 1;
    this.invalidatePrivateOperations({ saveDraft: true, closeDialogs: true });
    if (!this.active) return;
    this.active = false;
    this.wall.classList.remove("is-active");
    window.removeEventListener("resize", this.boundViewport);
    window.removeEventListener("scroll", this.boundViewport);
    window.visualViewport?.removeEventListener?.("resize", this.boundVisualViewport);
    window.visualViewport?.removeEventListener?.("scroll", this.boundVisualViewport);
    window.removeEventListener("online", this.boundOnline);
    window.removeEventListener("offline", this.boundOnline);
    document.removeEventListener("visibilitychange", this.boundVisibility);
    this.cancelGrowthIntent({ restoreFocus: false, announce: false, restoreView: false });
    void this.stopRealtime();
    if (this.pollTimer) window.clearInterval(this.pollTimer);
    if (this.eventTimer) window.clearTimeout(this.eventTimer);
    if (this.eventRetryTimer) window.clearTimeout(this.eventRetryTimer);
    if (this.listReconcileTimer) window.clearTimeout(this.listReconcileTimer);
    if (this.renderFrame) cancelAnimationFrame(this.renderFrame);
    if (this.focusFrame) cancelAnimationFrame(this.focusFrame);
    if (this.growthFitFrame) cancelAnimationFrame(this.growthFitFrame);
    this.growthFitFrame = null;
    for (const event of [...this.pendingEvents.values(), ...this.retryEvents.values(), ...this.realtimeDrainQueue.values(), ...this.consumingEvents.values()]) {
      if (event?.eventKey) this.seenRealtimeEvents.delete(event.eventKey);
      if (validCoordinate(Number(event?.q), Number(event?.r))) {
        const changedChunk = chunkFor(Number(event.q), Number(event.r));
        this.loadedChunks.delete(changedChunk.key);
        if (this.inflightChunks.has(changedChunk.key)) this.dirtyChunks.add(changedChunk.key);
      }
    }
    // Realtime is intentionally disconnected while the Wall route is inactive,
    // so every return must revalidate cached visible chunks even if no queued
    // event was present at the moment of deactivation.
    this.forceRefreshOnActivate = true;
    this.pollTimer = null;
    this.eventTimer = null;
    this.eventRetryTimer = null;
    this.listReconcileTimer = null;
    this.renderFrame = null;
    this.stopMomentum();
    this.viewportRefreshPromise = null;
    this.viewportRefreshDirty = false;
    this.viewportRefreshForce = false;
    this.realtimeDrainPromise = null;
    this.realtimeDrainDirty = false;
    this.focusFrame = null;
    this.pendingEvents.clear();
    this.retryEvents.clear();
    this.realtimeDrainQueue?.clear?.();
    this.consumingEvents.clear();
    this.discardAccessibleListWindow();
  }

  isActivationCurrent(generation) {
    return this.active && this.mounted && generation === this.activationGeneration;
  }

  isContextCurrent(activationGeneration, identityGeneration) {
    return this.isActivationCurrent(activationGeneration)
      && identityGeneration === this.identityGeneration;
  }

  closeDialogImmediately(dialog) {
    if (!dialog) return;
    if (typeof dialog.close === "function" && dialog.open) dialog.close();
    else dialog.removeAttribute("open");
    this.dialogOpeners.delete(dialog);
    this.dialogFocusAnchors.delete(dialog);
  }

  invalidatePrivateOperations(options = {}) {
    if (options.saveDraft && this.composeState) {
      if (this.composeState.editing) this.saveEditDraft();
      else this.saveDraft();
    }
    this.placementIntentGeneration += 1;
    this.focusGeneration += 1;
    this.composeGeneration += 1;
    this.readerGeneration += 1;
    this.watchGeneration += 1;
    this.watchQueueRequestGeneration += 1;
    this.watchCaseRequestGeneration += 1;
    this.watchActionGeneration += 1;
    this.operatorProbeGeneration += 1;
    this.listRequestGeneration += 1;
    this.composeBusy = false;
    this.readerBusy = false;
    this.watchBusy = false;
    this.watchLoadingGeneration = 0;
    this.composeState = null;
    this.readerCell = null;
    this.reportRequest = null;
    this.operatorCaseRequest = null;
    this.watchActionRequest = null;
    this.watchCase = null;
    this.operator = null;
    const watchButton = this.root.querySelector?.('[data-wall-action="watch"]');
    if (watchButton) watchButton.hidden = true;
    if (this.watchPollTimer) window.clearInterval(this.watchPollTimer);
    this.watchPollTimer = null;
    this.watchCases = [];
    this.watchCursor = null;
    this.watchHasMore = false;
    this.clearWatchDom();
    if (options.closeDialogs) {
      this.closeDialogImmediately(this.compose);
      this.closeDialogImmediately(this.reader);
      this.closeDialogImmediately(this.watch);
    }
  }

  preserveUnpersistedDrafts(scope = this.storageScope) {
    const hadPlacement = Boolean(this.draft || (this.composeState && !this.composeState.editing));
    const hadEdit = Boolean(this.editDraft || this.composeState?.editing);
    const placementSafe = !hadPlacement || this.saveDraft() !== false;
    const editSafe = !hadEdit || this.saveEditDraft() !== false;
    if (placementSafe && editSafe) return true;
    const quarantined = {
      draft: !placementSafe ? normalizedPlacementDraft(this.draft, true) : null,
      editDraft: !editSafe && this.editDraft ? { ...this.editDraft } : null,
    };
    if (scope) {
      const prior = this.quarantinedDrafts.get(scope) || {};
      this.quarantinedDrafts.set(scope, {
        draft: quarantined.draft || prior.draft || null,
        editDraft: quarantined.editDraft || prior.editDraft || null,
      });
    } else {
      this.orphanedPrivateDraft = quarantined;
    }
    return false;
  }

  resetIdentityState() {
    if (!this.mounted) return;
    this.publicCacheGeneration += 1;
    this.preserveUnpersistedDrafts(this.storageScope);
    this.invalidatePrivateOperations({ saveDraft: false, closeDialogs: true });
    this.cancelGrowthIntent({ restoreFocus: false, announce: false, restoreView: false });
    this.identityGeneration += 1;
    this.scopeRequestGeneration += 1;
    this.storageScope = null;
    this.cells.clear();
    this.cellsById.clear();
    this.conversations.clear();
    for (const node of this.nodes.values()) node.remove();
    this.nodes.clear();
    for (const line of this.connectionNodes?.values?.() || []) line.remove();
    this.connectionNodes?.clear?.();
    this.loadedChunks.clear();
    this.chunkAccess.clear();
    this.cellChunks.clear();
    this.dirtyChunks.clear();
    this.inflightChunks.clear();
    this.pendingEvents.clear();
    this.retryEvents.clear();
    this.listReconcileEvents.clear();
    this.realtimeDrainQueue?.clear?.();
    this.consumingEvents.clear();
    this.realtimeDrainPromise = null;
    this.realtimeDrainDirty = false;
    if (this.eventTimer) window.clearTimeout(this.eventTimer);
    if (this.eventRetryTimer) window.clearTimeout(this.eventRetryTimer);
    if (this.listReconcileTimer) window.clearTimeout(this.listReconcileTimer);
    this.eventTimer = null;
    this.eventRetryTimer = null;
    this.listReconcileTimer = null;
    this.seenRealtimeEvents.clear();
    this.listCells = [];
    this.listBeforeSeq = null;
    this.listHasMore = true;
    this.listConversationId = null;
    this.listWindowRevision = 0;
    this.wallWindowRevision = 0;
    this.bounds = { ...this.bounds, viewerRevision: 0 };
    this.listRequestGeneration += 1;
    this.seen.clear();
    this.draft = null;
    this.editDraft = null;
    this.draftPersistenceFailed = false;
    this.editDraftPersistenceFailed = false;
    this.draftFromHandoff = false;
    this.selected = { q: 0, r: 0 };
    this.cameraX = 0;
    this.hasActivated = false;
    if (this.composeBody) this.composeBody.value = "";
    this.hideComposeError();
    const reportReason = this.root.querySelector?.("[data-wall-report-reason]");
    const operatorReason = this.root.querySelector?.("[data-wall-operator-reason]");
    if (reportReason) reportReason.value = "";
    if (operatorReason) operatorReason.value = "";
    if (this.watchReason) this.watchReason.value = "";
    this.listItems?.replaceChildren();
    this.render();
    this.updateNewCount();
    this.updateTarget();
    this.updatePlaneTransform();
    this.updateResumeDraftAction();
  }

  clearWatchDom() {
    this.watchActionRequest = null;
    if (this.watchQueue) this.watchQueue.replaceChildren();
    if (this.watchDetail) this.watchDetail.hidden = true;
    const clearText = (selector) => {
      const element = this.root.querySelector?.(selector);
      if (element) element.textContent = "";
    };
    clearText("[data-wall-watch-status]");
    clearText("[data-wall-watch-meta]");
    clearText("[data-wall-watch-evidence]");
    clearText("[data-wall-watch-current]");
    const reports = this.root.querySelector?.("[data-wall-watch-reports]");
    const events = this.root.querySelector?.("[data-wall-watch-events]");
    const actions = this.root.querySelector?.("[data-wall-watch-actions]");
    const more = this.root.querySelector?.("[data-wall-watch-more]");
    reports?.replaceChildren();
    events?.replaceChildren();
    actions?.replaceChildren();
    if (more) more.hidden = true;
    if (this.watchReason) this.watchReason.value = "";
  }

  destroy() {
    this.deactivate();
    this.eventAbort?.abort();
    this.eventAbort = null;
    if (this.renderFrame) cancelAnimationFrame(this.renderFrame);
    if (this.focusFrame) cancelAnimationFrame(this.focusFrame);
    if (this.announceFrame) cancelAnimationFrame(this.announceFrame);
    if (this.eventTimer) window.clearTimeout(this.eventTimer);
    if (this.eventRetryTimer) window.clearTimeout(this.eventRetryTimer);
    if (this.listReconcileTimer) window.clearTimeout(this.listReconcileTimer);
    if (this.glideTimer) window.clearTimeout(this.glideTimer);
    this.trayObserver?.disconnect?.();
    this.trayObserver = null;
    this.stopMomentum();
    this.renderFrame = null;
    this.glideTimer = null;
    this.focusFrame = null;
    this.announceFrame = null;
    this.eventTimer = null;
    this.eventRetryTimer = null;
    this.listReconcileTimer = null;
    this.pendingEvents.clear();
    this.retryEvents.clear();
    this.listReconcileEvents.clear();
    this.realtimeDrainQueue?.clear?.();
    this.consumingEvents.clear();
    this.inflightChunks.clear();
    this.connectionNodes?.clear?.();
    this.nodes.clear();
    this.root.replaceChildren();
    this.mounted = false;
  }

  prepareIdentityHandoff() {
    if (this.composeState?.editing || this.editDraft) {
      if (this.saveEditDraft() === false) {
        this.toast("Account switching stopped because this edit draft cannot be persisted safely.");
        return false;
      }
      if (!this.draft && (!this.composeState || this.composeState.editing)) {
        this.cancelIdentityHandoff();
        return null;
      }
    }
    const persisted = this.composeState && !this.composeState.editing ? this.saveDraft() : (this.draft ? this.saveDraft() : true);
    const draft = normalizedPlacementDraft(this.draft, true);
    if (!draft) {
      this.cancelIdentityHandoff();
      return null;
    }
    if (!this.storageScope) {
      this.toast("This draft is not attached to a verified Hive identity yet. Finish or copy it before switching accounts.");
      return false;
    }
    const approved = window.confirm("Carry your unfinished Wall draft into the saved account you are opening? Choose Cancel to keep it private to the current Hive identity.");
    if (!approved) {
      this.cancelIdentityHandoff();
      if (persisted === false) {
        this.toast("Account switching stopped because the draft cannot stay safely with this identity in this browser.");
        return false;
      }
      this.toast("The draft stays private to the Hive identity that started it.");
      return null;
    }
    const stored = safeSet(this.handoffStorage, DRAFT_HANDOFF_KEY, {
      fromScope: this.storageScope,
      createdAt: Date.now(),
      sourceApproved: true,
      draft,
    });
    if (!stored) this.toast("This browser could not safely hand the draft to another account. It remains with the current identity.");
    return stored;
  }

  prepareIdentityExit() {
    this.cancelIdentityHandoff();
    if (!this.draft && !this.editDraft && !this.composeState) return true;
    if (!this.storageScope || !this.preserveUnpersistedDrafts(this.storageScope)) {
      this.toast("Starting fresh stopped because this browser cannot keep the unfinished Wall draft private to this identity.");
      return false;
    }
    return true;
  }

  cancelIdentityHandoff() {
    try { this.handoffStorage.removeItem(DRAFT_HANDOFF_KEY); } catch { /* no-op */ }
  }

  hasDraftHandoff() {
    const handoff = safeJson(this.handoffStorage, DRAFT_HANDOFF_KEY, null);
    const age = Date.now() - Number(handoff?.createdAt || 0);
    const valid = Boolean(handoff?.sourceApproved === true
      && typeof handoff.fromScope === "string"
      && normalizedPlacementDraft(handoff.draft, true)
      && age >= 0
      && age <= DRAFT_HANDOFF_TTL_MS);
    if (!valid && handoff) this.cancelIdentityHandoff();
    return valid;
  }

  async restoreDraftHandoff(priorScope) {
    const expectedScope = this.storageScope;
    const expectedIdentityGeneration = this.identityGeneration;
    const handoff = safeJson(this.handoffStorage, DRAFT_HANDOFF_KEY, null);
    if (!handoff) return false;
    const removeHandoff = () => {
      try { this.handoffStorage.removeItem(DRAFT_HANDOFF_KEY); } catch { /* no-op */ }
    };
    const age = Date.now() - Number(handoff.createdAt || 0);
    const draft = normalizedPlacementDraft(handoff.draft, true);
    if (handoff.sourceApproved !== true || !draft || age < 0 || age > DRAFT_HANDOFF_TTL_MS) {
      removeHandoff();
      return false;
    }
    const fromScope = typeof handoff.fromScope === "string" ? handoff.fromScope : "";
    if (!fromScope) {
      removeHandoff();
      return false;
    }
    if (fromScope && fromScope === this.storageScope) {
      if (this.draft) {
        removeHandoff();
        return true;
      }
      const sameScopeKey = this.scopedStorageKey(DRAFT_KEY);
      if (sameScopeKey && safeSet(this.storage, sameScopeKey, draft)) {
        this.draft = draft;
        this.draftFromHandoff = false;
        removeHandoff();
        this.toast("Your unfinished Wall draft was recovered with a new safe request.");
        this.updateResumeDraftAction();
        return true;
      }
      this.draft = draft;
      this.draftFromHandoff = false;
      removeHandoff();
      this.toast("Your handoff draft is open in this tab. This browser cannot store it locally, so copy it before reloading.");
      this.updateResumeDraftAction();
      return true;
    }
    if (this.draft) {
      removeHandoff();
      this.toast("This saved account already has an unfinished wall draft. The other draft remains with the browser identity that started it.");
      return false;
    }
    if (expectedScope !== this.storageScope || expectedIdentityGeneration !== this.identityGeneration) return false;
    if (!this.storageScope) return false;
    const key = this.scopedStorageKey(DRAFT_KEY);
    if (!key || !safeSet(this.storage, key, draft)) {
      this.draft = draft;
      this.draftFromHandoff = false;
      removeHandoff();
      this.toast("The approved draft is open in this tab. This browser cannot store it locally, so copy it before reloading.");
      this.updateResumeDraftAction();
      return true;
    }
    this.draft = draft;
    this.draftFromHandoff = false;
    if (fromScope) {
      try { this.storage.removeItem(`${DRAFT_KEY}:${fromScope}`); } catch { /* no-op */ }
      this.quarantinedDrafts.delete(fromScope);
    }
    removeHandoff();
    this.toast("Draft restored for this saved account with a new safe request.");
    this.updateResumeDraftAction();
    return true;
  }

  async resumeAfterIdentity(options = {}) {
    const activationGeneration = this.activationGeneration;
    if (!this.isActivationCurrent(activationGeneration)) return false;
    const priorScope = this.storageScope;
    await this.syncStorageScope(false);
    if (!this.isActivationCurrent(activationGeneration)) return false;
    const identityGeneration = this.identityGeneration;
    await this.restoreDraftHandoff(priorScope);
    if (!this.isContextCurrent(activationGeneration, identityGeneration)) return false;
    try {
      await this.startRealtime();
    } catch {
      this.unsubscribe = null;
      this.transportState = "polling";
      this.setConnectionState(navigator.onLine ? "polling" : "offline");
    }
    if (!this.isContextCurrent(activationGeneration, identityGeneration)) return false;
    void this.probeOperator();
    if (!this.active || !this.draft || !validCoordinate(this.draft.q, this.draft.r)) return false;
    return this.resumeCurrentDraft();
  }

  async resumeCurrentDraft() {
    const activationGeneration = this.activationGeneration;
    const identityGeneration = this.identityGeneration;
    if (!this.isContextCurrent(activationGeneration, identityGeneration)) return false;
    const draft = normalizedPlacementDraft(this.draft);
    if (!draft) return false;
    this.selected = { q: draft.q, r: draft.r };
    this.updateTarget();
    this.revealCoordinate(draft.q, draft.r, "auto");
    const context = await this.hydratePlacement(draft.q, draft.r);
    if (!this.isContextCurrent(activationGeneration, identityGeneration)) return false;
    const groups = context.ok ? (context.groups || adjacentConversationGroups(this.cells, draft.q, draft.r, this.conversations)) : [];
    const isolatedIndependent = context.ok && groups.length === 0 && !draft.choiceExplicit;
    this.openComposer({
      ...draft,
      choices: context.ok ? context.choices : [],
      groups,
      intent: isolatedIndependent ? "independent" : draft.intent,
      choiceExplicit: draft.choiceExplicit || isolatedIndependent,
      placementVerified: context.ok,
      requiresConversationChoice: draft.requiresConversationChoice
        || (!draft.choiceExplicit && context.ok && groups.length > 0),
      choiceMade: Boolean(isolatedIndependent || (draft.choiceMade && draft.intent)),
    });
    if (!context.ok) {
      this.showComposeError("The exact comb and draft are open, but the Hive must reconnect and verify the nearby conversations before it can be placed.");
    }
    return true;
  }

  updateResumeDraftAction() {
    const button = this.root.querySelector?.('[data-wall-action="resume"]');
    if (!button) return;
    const draft = normalizedPlacementDraft(this.draft, true);
    button.hidden = !draft;
    if (draft) button.textContent = `Resume draft at ${draft.q}, ${draft.r}`;
  }

  setConnectionState(state) {
    if (!this.wall) return;
    const previous = this.wall.dataset.connection;
    this.wall.dataset.connection = state;
    this.updatePersistentStatus?.();
    if (previous === state) return;
    if (state === "offline") this.announce("Offline. The last loaded wall stays visible and drafts remain on this device.");
  }

  updatePersistentStatus() {
    const status = this.root.querySelector?.("[data-wall-state-status]");
    if (!status) return;
    const connection = this.wall?.dataset?.connection || this.transportState || "polling";
    const connectionCopy = {
      live: "Live updates connected.",
      polling: "Checking periodically for shared changes.",
      reconnecting: "Reconnecting; the last verified Wall remains visible.",
      stale: "Connection interrupted; the last verified Wall remains visible.",
      offline: "Offline; the last verified Wall remains visible and drafts stay on this device.",
    }[connection] || "Checking the shared Wall.";
    const writeCopy = this.bounds?.readOnly
      ? "Reading is open. New combs remain closed until Hive Watch is staffed and verified."
      : "Reading and new combs are open.";
    setTextIfChanged(status, `${connectionCopy} ${writeCopy}`);
  }

  async stopRealtime() {
    const unsubscribe = this.unsubscribe;
    this.unsubscribe = null;
    const prior = this.realtimeTeardown;
    const next = Promise.resolve(prior)
      .catch(() => undefined)
      .then(() => (typeof unsubscribe === "function" ? unsubscribe() : undefined));
    this.realtimeTeardown = Promise.resolve(next).catch(() => undefined);
    await this.realtimeTeardown;
  }

  async startRealtime() {
    if (!this.active) return;
    const generation = ++this.realtimeGeneration;
    await this.stopRealtime();
    if (!this.active || generation !== this.realtimeGeneration) return;
    this.unsubscribe = this.backend.subscribeWallChanges(
      (event) => {
        if (this.active && generation === this.realtimeGeneration) this.queueRealtimeEvent(event);
      },
      (status) => {
        if (!this.active || generation !== this.realtimeGeneration) return;
        this.transportState = status === "SUBSCRIBED"
          ? "live"
          : status === "POLLING"
            ? "polling"
            : "reconnecting";
        this.setConnectionState(
          navigator.onLine ? (this.dataStale ? "stale" : this.transportState) : "offline",
        );
      },
    );
  }

  scopedStorageKey(base) {
    return this.storageScope ? `${base}:${this.storageScope}` : null;
  }

  async syncStorageScope(create) {
    const requestGeneration = ++this.scopeRequestGeneration;
    let nextScope = null;
    try {
      if (typeof this.backend.wallIdentityKey === "function") {
        nextScope = await this.backend.wallIdentityKey(Boolean(create));
      }
    } catch {
      this.dataStale = true;
      this.setConnectionState(navigator.onLine ? "stale" : "offline");
      return this.storageScope;
    }
    if (requestGeneration !== this.scopeRequestGeneration) return this.storageScope;
    if (nextScope === this.storageScope) return nextScope;
    const priorScope = this.storageScope;
    this.preserveUnpersistedDrafts(priorScope);
    this.invalidatePrivateOperations({ saveDraft: false, closeDialogs: true });
    this.identityGeneration += 1;
    this.publicCacheGeneration += 1;
    this.storageScope = nextScope;
    this.cells.clear();
    this.cellsById.clear();
    this.conversations.clear();
    for (const node of this.nodes.values()) node.remove();
    this.nodes.clear();
    for (const line of this.connectionNodes?.values?.() || []) line.remove();
    this.connectionNodes?.clear?.();
    this.loadedChunks.clear();
    this.chunkAccess.clear();
    this.cellChunks.clear();
    this.dirtyChunks.clear();
    this.inflightChunks.clear();
    if (this.eventTimer) window.clearTimeout(this.eventTimer);
    if (this.eventRetryTimer) window.clearTimeout(this.eventRetryTimer);
    if (this.listReconcileTimer) window.clearTimeout(this.listReconcileTimer);
    this.eventTimer = null;
    this.eventRetryTimer = null;
    this.listReconcileTimer = null;
    this.pendingEvents.clear();
    this.retryEvents.clear();
    this.realtimeDrainQueue?.clear?.();
    this.consumingEvents.clear();
    this.realtimeDrainPromise = null;
    this.realtimeDrainDirty = false;
    this.listReconcileEvents.clear();
    this.seenRealtimeEvents.clear();
    this.listCells = [];
    this.listBeforeSeq = null;
    this.listHasMore = true;
    this.listConversationId = null;
    this.listWindowRevision = 0;
    this.wallWindowRevision = 0;
    this.bounds = { ...this.bounds, viewerRevision: 0 };
    this.listRequestGeneration += 1;
    this.readerCell = null;
    this.reportRequest = null;
    const draftKey = this.scopedStorageKey(DRAFT_KEY);
    const editKey = this.scopedStorageKey(EDIT_DRAFT_KEY);
    const seenKey = this.scopedStorageKey(SEEN_KEY);
    const quarantined = nextScope ? this.quarantinedDrafts.get(nextScope) : null;
    this.draft = (draftKey ? safeJson(this.storage, draftKey, null) : null) || quarantined?.draft || null;
    this.editDraft = (editKey ? safeJson(this.storage, editKey, null) : null) || quarantined?.editDraft || null;
    if (nextScope && quarantined) this.quarantinedDrafts.delete(nextScope);
    const seen = seenKey ? safeJson(this.storage, seenKey, []) : [];
    this.seen = new Set(Array.isArray(seen) ? seen : []);
    this.draftPersistenceFailed = false;
    this.editDraftPersistenceFailed = false;
    this.draftFromHandoff = false;
    this.render();
    this.updateResumeDraftAction();
    return nextScope;
  }

  updateVisualViewport() {
    if (!this.wall) return;
    const rootStyles = getComputedStyle(document.documentElement);
    const sharedHeight = rootStyles.getPropertyValue("--hive-vv-height").trim();
    const sharedInset = rootStyles.getPropertyValue("--hive-keyboard-inset").trim();
    if (sharedHeight) {
      this.wall.style.setProperty("--honey-visual-height", "var(--hive-vv-height)");
      this.wall.style.setProperty("--honey-keyboard-offset", sharedInset ? "var(--hive-keyboard-inset)" : "0px");
      return;
    }
    const viewport = window.visualViewport;
    const height = viewport ? viewport.height : window.innerHeight;
    const keyboardOffset = viewport ? Math.max(0, window.innerHeight - viewport.height - viewport.offsetTop) : 0;
    this.wall.style.setProperty("--honey-visual-height", `${Math.max(1, height)}px`);
    this.wall.style.setProperty("--honey-keyboard-offset", `${keyboardOffset}px`);
  }

  async refreshBounds() {
    const identityGeneration = this.identityGeneration;
    const activationGeneration = this.activationGeneration;
    const requestGeneration = ++this.boundsRequestGeneration;
    const contextCurrent = () => (
      this.mounted
      && identityGeneration === this.identityGeneration
      && activationGeneration === this.activationGeneration
    );
    try {
      const incoming = await this.backend.wallBounds();
      if (!contextCurrent()) return this.bounds;
      if (!incoming || !Number.isSafeInteger(Number(incoming.revision))) throw new Error("The wall bounds were incomplete.");
      const viewerAdvanced = this.viewerProjectionAdvanced(incoming);
      if (!this.applyAuthoritativeBounds(incoming)) return this.bounds;
      if (viewerAdvanced && this.hasPublicProjection()) {
        // Private report visibility changes do not alter the shared Wall
        // revision. A per-viewer monotonic token makes another tab/device's
        // report invalidate every retained page and offscreen chunk safely.
        this.invalidatePublicProjectionCache({ resetConversation: false });
      }
      this.boundsSuccessGeneration = Math.max(this.boundsSuccessGeneration, requestGeneration);
      this.lastGoodAt = Date.now();
      this.dataStale = false;
      this.setConnectionState(navigator.onLine ? this.transportState : "offline");
      return this.bounds;
    } catch (error) {
      if (!contextCurrent() || requestGeneration < this.boundsSuccessGeneration) return this.bounds;
      this.dataStale = true;
      this.setConnectionState(navigator.onLine ? "stale" : "offline");
      if (!this.lastGoodAt) this.announce(`The shared wall is unavailable: ${error.message}`);
      return this.bounds;
    }
  }

  applyAuthoritativeBounds(incoming) {
    const incomingRevision = Number(incoming?.revision);
    if (!Number.isSafeInteger(incomingRevision) || incomingRevision < 0) return false;
    const incomingViewerRevision = Number(incoming?.viewerRevision) || 0;
    if (!Number.isSafeInteger(incomingViewerRevision) || incomingViewerRevision < 0) return false;
    const currentRevision = Number(this.bounds.revision) || 0;
    const currentViewerRevision = Number(this.bounds.viewerRevision) || 0;
    const incomingUpdatedAt = Number(incoming.updatedAt) || 0;
    const currentUpdatedAt = Number(this.bounds.updatedAt) || 0;
    if (incomingRevision < currentRevision
      || incomingViewerRevision < currentViewerRevision
      || (incomingRevision === currentRevision && incomingUpdatedAt < currentUpdatedAt)) return false;
    const sameSnapshot = incomingRevision === currentRevision && incomingUpdatedAt === currentUpdatedAt;
    this.bounds = {
      revision: incomingRevision,
      viewerRevision: incomingViewerRevision,
      radius: Math.max(this.bounds.radius, clamp(Number(incoming.radius) || 5, 5, WALL_LIMITS.maxRadius)),
      messageCount: Math.max(this.bounds.messageCount, Math.max(0, Number(incoming.messageCount) || 0)),
      // When two payloads claim the exact same ordering key but disagree,
      // remain closed. A strictly newer state timestamp may deliberately open.
      readOnly: sameSnapshot
        ? Boolean(this.bounds.readOnly || incoming.readOnly)
        : Boolean(incoming.readOnly),
      updatedAt: incomingUpdatedAt || currentUpdatedAt || Date.now(),
    };
    // The world is centred on the first comb, so growth expands symmetrically
    // around the camera. Nothing under the viewer shifts and no scroll
    // compensation is needed the way it was when the plane drove page scroll.
    this.updateWorldSize();
    this.updateSummary();
    return true;
  }

  viewerProjectionAdvanced(incoming) {
    const next = Number(incoming?.viewerRevision) || 0;
    const current = Number(this.bounds.viewerRevision) || 0;
    return Number.isSafeInteger(next) && next > current;
  }

  updateSummary() {
    if (!this.summary) return;
    const messages = this.bounds.messageCount === 1 ? "1 comb" : `${this.bounds.messageCount.toLocaleString()} combs`;
    setTextIfChanged(this.summary, `${messages} · wall revision ${this.bounds.revision.toLocaleString()}${this.bounds.readOnly ? " · reading open while Hive Watch is verified" : ""}`);
    this.wall?.toggleAttribute("data-read-only", this.bounds.readOnly);
    const add = this.root.querySelector('[data-wall-action="add"]');
    if (add) {
      add.disabled = this.bounds.readOnly;
      add.title = this.bounds.readOnly ? "Messages open after Hive Watch verification" : "Add a message to an open comb";
    }
    const edgeNote = this.root.querySelector("[data-wall-edge-note]");
    if (edgeNote) setTextIfChanged(edgeNote, this.bounds.readOnly
      ? "The Wall is open to read while Hive Watch is verified. New combs will open soon."
      : "Click any open place in the honeycomb to leave a message.");
    const composeSubmit = this.root.querySelector("[data-wall-compose-submit]");
    if (composeSubmit) {
      composeSubmit.disabled = this.bounds.readOnly
        || this.composeBusy
        || Boolean(this.composeState && !this.composeState.editing
          && (!this.composeState.choiceMade || !PLACEMENT_INTENTS.has(this.composeState.intent)));
    }
    this.updatePersistentStatus();
    if (this.readerCell) this.paintReaderCell(this.readerCell, { preserveForms: true });
  }

  captureAccessibleListView(preserveScroll = false) {
    const activeElement = globalThis.document?.activeElement || null;
    const focusedControl = activeElement && this.list?.contains(activeElement)
      ? activeElement.dataset?.wallListFocus || null
      : null;
    return {
      focusKey: focusedControl,
      focusTop: focusedControl && typeof activeElement.getBoundingClientRect === "function"
        ? activeElement.getBoundingClientRect().top
        : null,
      preserveScroll: Boolean(preserveScroll),
      scrollY: preserveScroll ? Number(globalThis.window?.scrollY) || 0 : null,
    };
  }

  restoreAccessibleListView(viewState, requestGeneration, conversationId) {
    if (!viewState) return;
    const restore = () => {
      if (requestGeneration !== this.listRequestGeneration
        || conversationId !== this.listConversationId || this.mode !== "list") return;
      const candidate = viewState.focusKey
        ? this.list?.querySelector(`[data-wall-list-focus="${escapeSelectorValue(viewState.focusKey)}"]`)
        : null;
      const control = candidate && !candidate.hidden && !candidate.disabled && !candidate.closest?.("[hidden]")
        ? candidate
        : null;
      if (control) {
        control.focus({ preventScroll: true });
        if (viewState.focusTop != null && typeof globalThis.window?.scrollBy === "function") {
          const topDelta = control.getBoundingClientRect().top - viewState.focusTop;
          if (topDelta) globalThis.window.scrollBy({ top: topDelta, behavior: "auto" });
        }
      } else {
        if (viewState.focusKey) {
          const fallback = this.list?.querySelector("[data-wall-list-items] button:not([hidden]):not(:disabled), button:not([hidden]):not(:disabled)") || this.list;
          fallback?.focus?.({ preventScroll: true });
          if (fallback && viewState.focusTop != null
            && typeof fallback.getBoundingClientRect === "function"
            && typeof globalThis.window?.scrollBy === "function") {
            const topDelta = fallback.getBoundingClientRect().top - viewState.focusTop;
            if (topDelta) globalThis.window.scrollBy({ top: topDelta, behavior: "auto" });
          } else fallback?.scrollIntoView?.({ block: "nearest", behavior: "auto" });
        } else if (viewState.preserveScroll && typeof globalThis.window?.scrollTo === "function") {
          globalThis.window.scrollTo({ top: viewState.scrollY, behavior: "auto" });
        }
      }
    };
    if (typeof globalThis.requestAnimationFrame === "function") requestAnimationFrame(restore);
    else restore();
  }

  invalidatePublicProjectionCache(options = {}) {
    this.publicCacheGeneration += 1;
    this.focusGeneration += 1;
    this.wallWindowRevision = 0;
    this.selectedCellKey = null;
    this.stage?.removeAttribute?.("aria-activedescendant");
    for (const node of this.nodes ? this.nodes.values() : []) {
      node.disabled = true;
      node.dataset.placeholder = "true";
      node.classList.add("is-refreshing");
      node.classList.remove("is-selected", "is-unseen", "is-mine", "is-viewer-hidden");
      node.removeAttribute("aria-current");
      node.setAttribute("aria-label", "Refreshing this comb's position");
      for (const part of node.querySelectorAll?.(".honey-comb-author,.honey-comb-body,.honey-comb-foot") || []) part.textContent = "";
    }
    for (const line of this.connectionNodes ? this.connectionNodes.values() : []) {
      line.classList.add("is-refreshing");
      line.removeAttribute("data-relation");
    }
    this.wall?.setAttribute?.("aria-busy", "true");
    this.cells?.clear?.();
    this.cellsById?.clear?.();
    this.cellChunks?.clear?.();
    this.loadedChunks?.clear?.();
    this.chunkAccess?.clear?.();
    this.dirtyChunks?.clear?.();
    this.inflightChunks?.clear?.();
    this.visibleChunkKeys?.clear?.();
    this.conversations?.clear?.();
    if (this.composeState && options.preserveComposer === false) {
      if (this.composeState.editing) this.saveEditDraft();
      else this.saveDraft();
      this.composeGeneration += 1;
      this.composeBusy = false;
      this.composeState = null;
      this.closeDialogImmediately(this.compose);
      this.updateResumeDraftAction();
    } else if (this.composeState && !this.composeState.editing) {
      // Public projections may be discarded while someone is writing. Keep
      // every draft byte and the exact coordinate open, but force submit to
      // re-hydrate the comb and its neighboring conversations.
      this.markComposerProjectionPending();
      this.draft = { ...this.composeState, body: this.composeBody?.value || this.composeState.body || "" };
      this.saveDraft();
    }
    if (this.readerCell || this.reader?.open || this.reader?.hasAttribute?.("open")) {
      this.closeReaderForProjectionRefresh();
    }
    this.discardAccessibleListWindow(options.resetConversation !== false);
  }

  publicProjectionAccountedRevision(mode = this.mode) {
    if (mode !== "list") return Number(this.wallWindowRevision) || 0;
    const retainedIds = new Set(this.listCells.map((cell) => cell?.id).filter(Boolean));
    return [...this.listReconcileEvents.values()].reduce((revision, event) => (
      retainedIds.has(event?.cellId)
        ? Math.max(revision, Number(event?.revision) || 0)
        : revision
    ), Number(this.listWindowRevision) || 0);
  }

  hasPublicProjection(mode = this.mode) {
    if (mode === "list") {
      return Boolean(this.listCells.length
        || this.listItems?.querySelector?.("[data-wall-list-row]")
        || this.cells?.size
        || this.cellsById?.size
        || this.conversations?.size
        || this.readerCell);
    }
    return Boolean(this.cells?.size || this.cellsById?.size || this.conversations?.size
      || this.loadedChunks?.size || this.nodes?.size || this.readerCell);
  }

  hasUnaccountedPublicRevision(revision, mode = this.mode) {
    const nextRevision = Number(revision);
    return Number.isSafeInteger(nextRevision)
      && this.hasPublicProjection(mode)
      && nextRevision > this.publicProjectionAccountedRevision(mode);
  }

  captureProjectionRebuild() {
    return {
      mode: this.mode,
      conversationId: this.listConversationId,
      listView: this.mode === "list" ? this.captureAccessibleListView(true) : null,
    };
  }

  async rebuildPublicProjection(rebuild) {
    if (!rebuild || rebuild.mode !== this.mode || !this.active || !this.mounted) return false;
    if (rebuild.mode === "list") {
      if (rebuild.conversationId !== this.listConversationId) return false;
      await this.refreshList(true, rebuild.listView);
      return this.mode === "list" && rebuild.conversationId === this.listConversationId;
    }
    return this.refreshVisible(true, 0);
  }

  refreshCurrentPublicProjection() {
    if (!this.active || !this.mounted) return;
    if (this.mode === "list") void this.refreshList(true);
    else void this.refreshVisible(true);
  }

  async rebuildAfterConfirmedWrite(rebuild = this.captureProjectionRebuild()) {
    this.invalidatePublicProjectionCache({ resetConversation: false });
    return this.rebuildPublicProjection(rebuild);
  }

  evictWallChunkProjection(chunkKey, options = {}) {
    const preserveGeometry = options.preserveGeometry === true;
    this.loadedChunks.delete(chunkKey);
    this.chunkAccess.delete(chunkKey);
    this.dirtyChunks.delete(chunkKey);
    let readerEvicted = false;
    const evictedIds = new Set();
    for (const [cellId, cellChunk] of [...this.cellChunks]) {
      if (cellChunk !== chunkKey) continue;
      evictedIds.add(cellId);
      const cell = this.cellsById.get(cellId);
      if (this.readerCell?.id === cellId) readerEvicted = true;
      if (cell && this.cells.get(cellKey(cell.q, cell.r))?.id === cellId) {
        this.cells.delete(cellKey(cell.q, cell.r));
        const node = this.nodes.get(cellKey(cell.q, cell.r));
        if (preserveGeometry && node) {
          node.disabled = true;
          node.dataset.placeholder = "true";
          node.classList.add("is-refreshing");
          node.classList.remove("is-selected", "is-unseen", "is-mine", "is-viewer-hidden");
          node.removeAttribute("aria-current");
          node.setAttribute("aria-label", "Refreshing this comb's position");
          for (const part of node.querySelectorAll(".honey-comb-author,.honey-comb-body,.honey-comb-foot")) part.textContent = "";
        } else {
          node?.remove();
          this.nodes.delete(cellKey(cell.q, cell.r));
        }
      }
      this.cellsById.delete(cellId);
      this.cellChunks.delete(cellId);
    }
    if (preserveGeometry) {
      for (const [key, line] of this.connectionNodes) {
        if (![...evictedIds].some((cellId) => key.includes(`:${cellId}`))) continue;
        line.classList.add("is-refreshing");
        line.removeAttribute("data-relation");
      }
      this.wall?.setAttribute?.("aria-busy", "true");
    }
    if (readerEvicted) this.closeReaderForProjectionRefresh();
  }

  evictPublicCellProjection(cellId) {
    const cell = this.cellsById.get(cellId) || this.listCells.find((item) => item?.id === cellId) || null;
    if (cell && this.cells.get(cellKey(cell.q, cell.r))?.id === cellId) {
      this.cells.delete(cellKey(cell.q, cell.r));
      const node = this.nodes.get(cellKey(cell.q, cell.r));
      node?.remove();
      this.nodes.delete(cellKey(cell.q, cell.r));
    }
    this.cellsById.delete(cellId);
    this.cellChunks.delete(cellId);
    this.listCells = this.listCells.filter((item) => item?.id !== cellId);
    if (this.readerCell?.id === cellId) this.closeReaderForProjectionRefresh();
  }

  closeReaderForProjectionRefresh() {
    if (!this.readerCell && !this.reader?.open && !this.reader?.hasAttribute?.("open")) return;
    const opener = this.dialogOpeners.get(this.reader) || null;
    const focusAnchor = this.dialogFocusAnchors.get(this.reader) || null;
    const body = this.root?.querySelector?.("[data-wall-reader-body]");
    setTextIfChanged(body, "Refreshing this comb from the shared Wall…");
    this.readerGeneration += 1;
    this.readerBusy = false;
    this.readerCell = null;
    this.reportRequest = null;
    this.operatorCaseRequest = null;
    this.closeDialogImmediately(this.reader);
    this.restoreWallFocus(opener, focusAnchor);
  }

  markComposerProjectionPending() {
    if (!this.composeState || this.composeState.editing) return;
    const hadConversationContext = Boolean(
      this.composeState.conversationId
      || this.composeState.replyToId
      || this.composeState.requiresConversationChoice
      || this.composeState.choices?.length
    );
    this.composeState.placementVerified = false;
    this.composeState.choices = [];
    // An uncertain/in-flight command must retain both its durable request ID
    // and exact payload. An explicit human choice is frozen too: live activity
    // may require its anchors to be revalidated, but may never silently erase
    // or reinterpret the selected relationship. Only an unchosen draft drops
    // stale adjacency so the next hydration asks again.
    const preservesFrozenChoice = Boolean(this.composeState.requestId || this.composeState.choiceExplicit);
    if (!preservesFrozenChoice) {
      this.composeState.conversationId = null;
      this.composeState.replyToId = null;
      this.composeState.choiceMade = false;
      this.composeState.choiceExplicit = false;
    }
    this.composeState.requiresConversationChoice = hadConversationContext && !preservesFrozenChoice;
    const choiceField = this.root?.querySelector?.("[data-wall-conversation-choices]");
    const choiceList = this.root?.querySelector?.("[data-wall-conversation-options]");
    if (!choiceField || !choiceList) return;
    const choiceHadFocus = Boolean(globalThis.document?.activeElement
      && choiceList.contains?.(globalThis.document.activeElement));
    choiceList.replaceChildren();
    choiceField.hidden = !hadConversationContext;
    if (hadConversationContext) {
      const pending = document.createElement("p");
      pending.className = "honey-choice-pending";
      pending.textContent = "Nearby conversations must reload before you choose where this comb belongs.";
      choiceList.append(pending);
      if (choiceHadFocus) {
        this.announce(pending.textContent);
        const restore = () => this.composeBody?.focus?.({ preventScroll: true });
        if (typeof globalThis.requestAnimationFrame === "function") requestAnimationFrame(restore);
        else restore();
      }
    }
  }

  updateWorldSize() {
    if (!this.plane || !this.stage) return;
    const radius = Math.max(0, Number(this.bounds?.radius) || 0);
    // Half-extents of the occupied hex field. |q|,|r|,|q+r| <= radius puts the
    // widest cell at (radius, 0) and the lowest at (0, radius).
    const halfWidth = axialToPixel(radius, 0, this.hexRadius).x + this.hexRadius;
    const halfHeight = radius * this.hexRadius * 1.5 + this.hexRadius;
    const width = halfWidth * 2 + WORLD_MARGIN * 2;
    const height = halfHeight * 2 + WORLD_MARGIN * 2;
    const originX = width / 2;
    const originY = height / 2;
    // Bounds are re-applied on every poll, usually with an unchanged radius.
    // Returning early keeps an idle wall completely still: no transform write,
    // no camera settle, and no refit scheduled behind an open growth picker.
    if (this.world.halfWidth === halfWidth && this.world.halfHeight === halfHeight) return;
    this.world = { halfWidth, halfHeight, width, height, originX, originY };
    this.plane.style.width = `${Math.ceil(width)}px`;
    this.plane.style.height = `${Math.ceil(height)}px`;
    this.plane.style.marginLeft = `${Math.ceil(width / -2)}px`;
    this.plane.style.setProperty("--honey-origin-x", `${Math.ceil(originX)}px`);
    this.plane.style.setProperty("--honey-origin-y", `${Math.ceil(originY)}px`);
    if (this.growthIntent) {
      // A growth picker owns the camera and is deliberately unclamped so an
      // edge cluster stays reachable. A bounds poll landing mid-choice must
      // refit it, never clamp it back and slide the open directions away.
      this.updatePlaneTransform();
      this.scheduleGrowthFit();
      return;
    }
    this.clampCamera();
    this.updatePlaneTransform();
  }

  stageSize() {
    if (!this.stage) return { width: Math.max(1, window.innerWidth), height: Math.max(1, window.innerHeight) };
    return {
      width: Math.max(1, this.stage.clientWidth || window.innerWidth),
      height: Math.max(1, this.stage.clientHeight || window.innerHeight),
    };
  }

  // The part of the stage the viewer can actually see, in stage coordinates.
  // Browser page-scale zoom makes the visual viewport a sub-rectangle of the
  // layout viewport, so aiming at the stage's own centre would point at pixels
  // that are off screen. Falls back to the whole stage when they coincide.
  visibleStageRect() {
    const size = this.stageSize();
    const whole = { left: 0, top: 0, right: size.width, bottom: size.height, width: size.width, height: size.height };
    const rect = this.stage?.getBoundingClientRect?.();
    const viewport = globalThis.window?.visualViewport;
    if (!rect || !viewport) return whole;
    const offsetLeft = Number(viewport.offsetLeft) || 0;
    const offsetTop = Number(viewport.offsetTop) || 0;
    const left = Math.max(0, offsetLeft - rect.left);
    const top = Math.max(0, offsetTop - rect.top);
    const right = Math.min(size.width, offsetLeft + (Number(viewport.width) || size.width) - rect.left);
    const bottom = Math.min(size.height, offsetTop + (Number(viewport.height) || size.height) - rect.top);
    if (right - left < 1 || bottom - top < 1) return whole;
    return { left, top, right, bottom, width: right - left, height: bottom - top };
  }

  // World coordinates are the same space axialToPixel() returns, with (0,0) at
  // the first comb. Stage coordinates are pixels inside the stage box, which
  // is also the visible viewport now that the plane no longer drives page
  // scroll. Every hit test, reveal, and clamp goes through this one pair.
  worldToStage(x, y) {
    const size = this.stageSize();
    return {
      x: size.width / 2 + Number(x) * this.zoom + this.cameraX,
      y: (this.world.originY + Number(y)) * this.zoom + this.cameraY,
    };
  }

  stageToWorld(x, y) {
    const size = this.stageSize();
    return {
      x: (Number(x) - size.width / 2 - this.cameraX) / this.zoom,
      y: (Number(y) - this.cameraY) / this.zoom - this.world.originY,
    };
  }

  // The box the combs actually occupy, in world pixels. A wall of eighteen
  // combs sits in a small patch of a radius-512 field, so framing and camera
  // travel follow the conversations rather than the empty coordinate space.
  contentExtent() {
    let minX = Infinity;
    let maxX = -Infinity;
    let minY = Infinity;
    let maxY = -Infinity;
    for (const cell of this.cells.values()) {
      const point = axialToPixel(cell.q, cell.r, this.hexRadius);
      if (point.x < minX) minX = point.x;
      if (point.x > maxX) maxX = point.x;
      if (point.y < minY) minY = point.y;
      if (point.y > maxY) maxY = point.y;
    }
    if (!Number.isFinite(minX)) {
      minX = -this.hexRadius;
      maxX = this.hexRadius;
      minY = -this.hexRadius;
      maxY = this.hexRadius;
    }
    // Always keep the first comb inside the frame; it is the wall's anchor and
    // the target of Center and Home.
    const halfCellX = this.hexRadius * SQRT_3 * 0.5;
    return {
      minX: Math.min(minX, 0) - halfCellX,
      maxX: Math.max(maxX, 0) + halfCellX,
      minY: Math.min(minY, 0) - this.hexRadius,
      maxY: Math.max(maxY, 0) + this.hexRadius,
    };
  }

  // A phone should open on a readable patch, not one empty coordinate or an
  // unreadably tiny map of the entire Hive. Frame the first comb and its
  // immediate neighbours; See it all remains available for the global view.
  initialPhoneExtent() {
    let minX = 0;
    let maxX = 0;
    let minY = 0;
    let maxY = 0;
    for (const cell of this.cells.values()) {
      if (hexDistance(cell.q, cell.r) > 1) continue;
      const point = axialToPixel(cell.q, cell.r, this.hexRadius);
      minX = Math.min(minX, point.x);
      maxX = Math.max(maxX, point.x);
      minY = Math.min(minY, point.y);
      maxY = Math.max(maxY, point.y);
    }
    const halfCellX = this.hexRadius * SQRT_3 * 0.5;
    return {
      minX: minX - halfCellX,
      maxX: maxX + halfCellX,
      minY: minY - this.hexRadius,
      maxY: maxY + this.hexRadius,
    };
  }

  // Where the camera may roam: the whole authoritative wall, not just the part
  // that happens to be loaded. Restricting travel to loaded combs would make
  // distant clusters permanently undiscoverable.
  cameraBounds() {
    return {
      minX: -this.world.halfWidth,
      maxX: this.world.halfWidth,
      minY: -this.world.halfHeight,
      maxY: this.world.halfHeight,
    };
  }

  zoomToFit(extent) {
    const size = this.stageSize();
    // A collapsed or not-yet-laid-out stage would compute a nonsense zoom and
    // strand the viewer at it once the real size arrives. Keep what we have.
    if (size.width < 2 || size.height < 2) return this.zoom;
    const pad = 28;
    const spanX = Math.max(1, extent.maxX - extent.minX + pad * 2);
    const spanY = Math.max(1, extent.maxY - extent.minY + pad * 2);
    return clamp(Math.min(size.width / spanX, size.height / spanY), WALL_LIMITS.minZoom, WALL_LIMITS.maxZoom);
  }

  // "See it all" frames the combs that exist, which is what a viewer means by
  // seeing everything. Pulling back further is still allowed, and that is what
  // reveals clusters sitting outside the loaded window.
  fitZoom() {
    return this.zoomToFit(this.contentExtent());
  }

  zoomFloor() {
    // The floor is the whole growable field, not the loaded combs, so zooming
    // out can always reach wall the viewer has not visited yet. A small wall
    // still never forces the viewer further out than 100%.
    const world = this.zoomToFit({
      minX: -this.world.halfWidth,
      maxX: this.world.halfWidth,
      minY: -this.world.halfHeight,
      maxY: this.world.halfHeight,
    });
    return clamp(Math.min(world, 1), WALL_LIMITS.minZoom, WALL_LIMITS.maxZoom);
  }

  clampZoom(value) {
    const next = Number(value);
    if (!Number.isFinite(next)) return this.zoom;
    return clamp(next, this.zoomFloor(), WALL_LIMITS.maxZoom);
  }

  // Convert the world point the viewer is looking at into camera offsets.
  cameraForFocus(x, y, zoom = this.zoom) {
    const size = this.stageSize();
    return {
      x: -Number(x) * zoom,
      y: size.height / 2 - (this.world.originY + Number(y)) * zoom,
    };
  }

  cameraFocus(zoom = this.zoom) {
    const size = this.stageSize();
    return {
      x: -this.cameraX / zoom,
      y: (size.height / 2 - this.cameraY) / zoom - this.world.originY,
    };
  }

  // Clamp the world point at the middle of the stage. An axis whose reachable
  // box is smaller than the stage locks to that box's centre, which is what
  // makes the fully zoomed-out view sit still instead of sliding around.
  clampCamera(zoom = this.zoom) {
    const size = this.stageSize();
    const box = this.cameraBounds();
    const content = this.contentExtent();
    const focus = this.cameraFocus(zoom);
    const overX = this.hexRadius * SQRT_3 * 0.75;
    const overY = this.hexRadius * 1.5 * 0.75;
    const halfViewX = size.width / 2 / zoom;
    const halfViewY = size.height / 2 / zoom;
    // An axis whose reachable box already fits on screen has nothing to pan
    // to, so it settles on the middle of the combs rather than the middle of
    // the coordinate space; a wall growing downward should not sit low in an
    // otherwise empty frame.
    const axis = (value, min, max, halfView, over, contentMid) => {
      if (max - min <= halfView * 2) return clamp(contentMid, min, max);
      return clamp(value, min + halfView - over, max - halfView + over);
    };
    const nextX = axis(Number.isFinite(focus.x) ? focus.x : 0, box.minX, box.maxX, halfViewX, overX, (content.minX + content.maxX) / 2);
    const nextY = axis(Number.isFinite(focus.y) ? focus.y : 0, box.minY, box.maxY, halfViewY, overY, (content.minY + content.maxY) / 2);
    const camera = this.cameraForFocus(nextX, nextY, zoom);
    this.cameraX = camera.x;
    this.cameraY = camera.y;
    return { x: this.cameraX, y: this.cameraY };
  }

  // Retained so existing callers that only settle the horizontal axis keep
  // working; both axes are clamped together because a zoom change moves both.
  clampCameraX() {
    return this.clampCamera().x;
  }

  detailTierFor(zoom = this.zoom) {
    return (DETAIL_TIERS.find((tier) => zoom >= tier.minZoom) || DETAIL_TIERS[DETAIL_TIERS.length - 1]).id;
  }

  updatePlaneTransform() {
    if (!this.plane) return;
    this.plane.style.transform = `translate3d(${this.cameraX}px, ${this.cameraY}px, 0) scale(${this.zoom})`;
    if (this.zoomOutput) this.zoomOutput.textContent = `${Math.round(this.zoom * 100)}%`;
    const tier = this.detailTierFor();
    if (tier !== this.detailTier) {
      this.detailTier = tier;
      this.wall?.setAttribute?.("data-wall-detail", tier);
    }
  }

  visibleCoordinates() {
    if (!this.stage || !this.plane) return { qMin: -4, qMax: 4, rMin: -4, rMax: 4, originX: 0 };
    const size = this.stageSize();
    const topLeft = this.stageToWorld(0, 0);
    const bottomRight = this.stageToWorld(size.width, size.height);
    const corners = [
      pixelToAxial(topLeft.x, topLeft.y, this.hexRadius),
      pixelToAxial(bottomRight.x, topLeft.y, this.hexRadius),
      pixelToAxial(topLeft.x, bottomRight.y, this.hexRadius),
      pixelToAxial(bottomRight.x, bottomRight.y, this.hexRadius),
    ];
    // Three rings of slack at 100%, as before, and proportionally more when
    // zoomed out, where a fixed ring count would be a sliver of the screen.
    // Never fewer than three: combs just outside the camera must stay mounted
    // so focus, links, and realtime updates can reach them.
    const overscan = clamp(Math.round(3 / Math.max(this.zoom, 0.05)), 3, 16);
    return {
      qMin: clamp(Math.min(...corners.map((item) => item.q)) - overscan, -this.bounds.radius, this.bounds.radius),
      qMax: clamp(Math.max(...corners.map((item) => item.q)) + overscan, -this.bounds.radius, this.bounds.radius),
      rMin: clamp(Math.min(...corners.map((item) => item.r)) - overscan, -this.bounds.radius, this.bounds.radius),
      rMax: clamp(Math.max(...corners.map((item) => item.r)) + overscan, -this.bounds.radius, this.bounds.radius),
      originX: this.world.originX,
    };
  }

  chunksForVisibleRegion() {
    const bounds = this.visibleCoordinates();
    const centre = this.stageToWorld(this.stageSize().width / 2, this.stageSize().height / 2);
    const centreAxial = pixelToAxial(centre.x, centre.y, this.hexRadius);
    const chunks = new Map();
    const ordered = [];
    for (let q = Math.floor(bounds.qMin / WALL_LIMITS.chunkSize) * WALL_LIMITS.chunkSize; q <= bounds.qMax; q += WALL_LIMITS.chunkSize) {
      for (let r = Math.floor(bounds.rMin / WALL_LIMITS.chunkSize) * WALL_LIMITS.chunkSize; r <= bounds.rMax; r += WALL_LIMITS.chunkSize) {
        const chunk = chunkFor(q, r);
        if (chunks.has(chunk.key)) continue;
        chunks.set(chunk.key, chunk);
        ordered.push(chunk);
      }
    }
    if (ordered.length <= MAX_VISIBLE_CHUNKS) return chunks;
    // Pulled far enough out that the viewport covers more wall than one pass
    // should fetch. Keep the chunks nearest the camera so the middle of the
    // view is coherent; the rest load as the viewer moves toward them.
    const distance = (chunk) => hexDistance(
      (chunk.qMin + chunk.qMax) / 2,
      (chunk.rMin + chunk.rMax) / 2,
      centreAxial.q,
      centreAxial.r,
    );
    ordered.sort((left, right) => distance(left) - distance(right));
    return new Map(ordered.slice(0, MAX_VISIBLE_CHUNKS).map((chunk) => [chunk.key, chunk]));
  }

  // Every unit of transient scheduled work, in one place. "Settled" has to mean
  // the camera glide and the realtime debounces too, not just the frames and
  // promises: a glide that is still armed will schedule a viewport refresh when
  // it lands, so treating the wall as idle while one is pending reads a legal
  // mid-settle moment as an idle loop. The two long-lived intervals (pollTimer,
  // watchPollTimer) are deliberately excluded — they are meant to outlive idle.
  pendingWork() {
    const pending = [];
    if (this.growthFitFrame) pending.push("growthFitFrame");
    if (this.renderFrame) pending.push("renderFrame");
    if (this.momentumFrame) pending.push("momentumFrame");
    if (this.focusFrame) pending.push("focusFrame");
    if (this.announceFrame) pending.push("announceFrame");
    if (this.glideTimer) pending.push("glideTimer");
    if (this.eventTimer) pending.push("eventTimer");
    if (this.eventRetryTimer) pending.push("eventRetryTimer");
    if (this.listReconcileTimer) pending.push("listReconcileTimer");
    if (this.viewportRefreshDirty) pending.push("viewportRefreshDirty");
    if (this.viewportRefreshPromise) pending.push("viewportRefreshPromise");
    if (this.realtimeDrainPromise) pending.push("realtimeDrainPromise");
    return pending;
  }

  async refreshVisible(force = false) {
    if (!this.active || this.mode !== "wall") return false;
    this.viewportRefreshDirty = true;
    this.viewportRefreshForce = this.viewportRefreshForce || Boolean(force);
    if (this.viewportRefreshPromise) return this.viewportRefreshPromise;
    const run = async () => {
      let result = false;
      let pass = 0;
      while (this.viewportRefreshDirty && pass < 2 && this.active && this.mode === "wall") {
        const passForce = this.viewportRefreshForce;
        this.viewportRefreshDirty = false;
        this.viewportRefreshForce = false;
        const refresh = typeof this.performVisibleRefresh === "function"
          ? this.performVisibleRefresh
          : HiveWallController.prototype.performVisibleRefresh;
        result = await refresh.call(this, passForce, 0);
        pass += 1;
      }
      return result;
    };
    const request = run().finally(() => {
      if (this.viewportRefreshPromise === request) this.viewportRefreshPromise = null;
      if (this.viewportRefreshDirty && this.active && this.mode === "wall") this.scheduleViewport();
    });
    this.viewportRefreshPromise = request;
    return request;
  }

  async performVisibleRefresh(force = false, retry = 0) {
    if (!this.active || this.mode !== "wall") return false;
    const identityGeneration = this.identityGeneration;
    await this.refreshBounds();
    if (!this.active || this.mode !== "wall" || identityGeneration !== this.identityGeneration) return false;
    const targetRevision = Number(this.bounds.revision) || 0;
    if (this.hasUnaccountedPublicRevision(targetRevision, "wall")) {
      // Polling observed a public mutation for which no complete broadcast
      // chain was reconciled. Remove every shared projection synchronously;
      // a failed reload may leave an empty Wall, never stale public words.
      this.invalidatePublicProjectionCache({ resetConversation: false });
    }
    const cacheGeneration = this.publicCacheGeneration;
    const chunks = this.chunksForVisibleRegion();
    this.visibleChunkKeys = new Set(chunks.keys());
    const results = await Promise.all([...chunks.values()]
      .map((chunk) => this.loadChunk(chunk, force, targetRevision)));
    if (!this.active || this.mode !== "wall" || identityGeneration !== this.identityGeneration) return false;
    if (cacheGeneration !== this.publicCacheGeneration) {
      if (retry < 1) return this.performVisibleRefresh(true, retry + 1);
      return false;
    }
    const complete = results.every((result) => result?.ok && Number(result.revision) === targetRevision);
    if (!complete) return false;
    this.wallWindowRevision = Math.max(Number(this.wallWindowRevision) || 0, targetRevision);
    this.wall?.setAttribute?.("aria-busy", "false");
    this.render();
    return true;
  }

  loadChunk(chunk, force = false, accountedRevision = null) {
    const expectedRevision = Number.isSafeInteger(Number(accountedRevision))
      ? Number(accountedRevision)
      : Number(this.bounds.revision) || 0;
    if (!force && this.loadedChunks.has(chunk.key)) {
      return Promise.resolve({ ok: true, revision: expectedRevision, cached: true });
    }
    if (this.inflightChunks.has(chunk.key)) {
      if (force) this.dirtyChunks.add(chunk.key);
      return this.inflightChunks.get(chunk.key);
    }
    let request;
    const identityGeneration = this.identityGeneration;
    const cacheGeneration = this.publicCacheGeneration;
    const initial = this.backend.wallRegion({ ...chunk, limit: 500 })
      .then((payload) => {
        if (!this.mounted || identityGeneration !== this.identityGeneration
          || cacheGeneration !== this.publicCacheGeneration) return { ok: false, stale: true };
        if (!payload || !Array.isArray(payload.cells) || !payload.bounds
          || !Number.isSafeInteger(Number(payload.bounds.revision))) throw new Error("The wall region was incomplete.");
        const payloadRevision = Number(payload.bounds.revision);
        const viewerAdvanced = this.viewerProjectionAdvanced(payload.bounds);
        if (!this.applyAuthoritativeBounds(payload.bounds)) throw new Error("This wall region returned an older snapshot.");
        if (viewerAdvanced && this.hasPublicProjection()) {
          this.invalidatePublicProjectionCache({ resetConversation: false });
          return { ok: false, advanced: true, viewerAdvanced: true, revision: payloadRevision };
        }
        if (payloadRevision < expectedRevision) throw new Error("This wall region was behind the requested snapshot.");
        if (payloadRevision > expectedRevision) {
          // A newer snapshot means another mutation landed after the viewport
          // target was chosen. Do not mix snapshots; invalidate the generation
          // so all sibling region promises become harmless and retry once.
          this.invalidatePublicProjectionCache({ resetConversation: false });
          return { ok: false, advanced: true, revision: payloadRevision };
        }
        for (const cell of payload.cells) this.mergeCell(cell);
        if (Array.isArray(payload.conversations)) {
          for (const conversation of payload.conversations) {
            if (conversation && conversation.id) this.conversations.set(conversation.id, conversation);
          }
        }
        this.loadedChunks.add(chunk.key);
        this.chunkAccess.set(chunk.key, Date.now());
        this.lastGoodAt = Date.now();
        this.pruneChunkCache();
        return { ok: true, revision: payloadRevision };
      })
      .catch((error) => {
        if (!this.mounted || identityGeneration !== this.identityGeneration
          || cacheGeneration !== this.publicCacheGeneration) return { ok: false, stale: true };
        this.dataStale = true;
        this.setConnectionState(navigator.onLine ? "stale" : "offline");
        if (!this.loadedChunks.has(chunk.key)) this.announce(`Could not load this part of the wall: ${error.message}`);
        return { ok: false, error };
      })
      .finally(() => {
        if (this.inflightChunks.get(chunk.key) === request) this.inflightChunks.delete(chunk.key);
      });
    request = initial.then((result) => {
      if (identityGeneration === this.identityGeneration
        && cacheGeneration === this.publicCacheGeneration
        && this.dirtyChunks.delete(chunk.key) && this.mounted) return this.loadChunk(chunk, true, expectedRevision);
      return result;
    });
    this.inflightChunks.set(chunk.key, request);
    return request;
  }

  async hydratePlacement(q, r) {
    const targetQ = Number(q);
    const targetR = Number(r);
    if (!validCoordinate(targetQ, targetR)) return { ok: false, code: "out_of_bounds", choices: [] };
    const identityGeneration = this.identityGeneration;
    const cacheGeneration = this.publicCacheGeneration;
    try {
      const payload = await this.backend.wallRegion({
        qMin: clamp(targetQ - 1, -WALL_LIMITS.maxRadius, WALL_LIMITS.maxRadius),
        qMax: clamp(targetQ + 1, -WALL_LIMITS.maxRadius, WALL_LIMITS.maxRadius),
        rMin: clamp(targetR - 1, -WALL_LIMITS.maxRadius, WALL_LIMITS.maxRadius),
        rMax: clamp(targetR + 1, -WALL_LIMITS.maxRadius, WALL_LIMITS.maxRadius),
        limit: 16,
      });
      if (identityGeneration !== this.identityGeneration) return { ok: false, code: "identity_changed", choices: [] };
      if (cacheGeneration !== this.publicCacheGeneration) return { ok: false, code: "wall_changed", choices: [] };
      if (!payload || !Array.isArray(payload.cells) || !payload.bounds
        || !Number.isSafeInteger(Number(payload.bounds.revision))) {
        throw new Error("The wall did not return a complete placement check.");
      }
      const payloadRevision = Number(payload.bounds.revision);
      const viewerAdvanced = this.viewerProjectionAdvanced(payload.bounds);
      if (!this.applyAuthoritativeBounds(payload.bounds)) return { ok: false, code: "wall_changed", choices: [] };
      if (viewerAdvanced && this.hasPublicProjection()) {
        const rebuild = this.captureProjectionRebuild();
        this.invalidatePublicProjectionCache({ resetConversation: false });
        void this.rebuildPublicProjection(rebuild);
        return { ok: false, code: "wall_changed", choices: [] };
      }
      if (this.hasUnaccountedPublicRevision(payloadRevision)) {
        const rebuild = this.captureProjectionRebuild();
        this.invalidatePublicProjectionCache({ resetConversation: false });
        void this.rebuildPublicProjection(rebuild);
        return { ok: false, code: "wall_changed", choices: [] };
      }
      for (const cell of payload.cells) this.mergeCell(cell);
      if (Array.isArray(payload.conversations)) {
        for (const conversation of payload.conversations) {
          if (conversation && conversation.id) this.conversations.set(conversation.id, conversation);
        }
      }
      this.lastGoodAt = Date.now();
      this.dataStale = false;
      this.render();
      if (!validCoordinate(targetQ, targetR, this.bounds.radius)) {
        return { ok: false, code: "out_of_bounds", choices: [] };
      }
      return {
        ok: true,
        occupied: this.cells.get(cellKey(targetQ, targetR)) || null,
        choices: conversationChoices(this.cells, targetQ, targetR, this.conversations),
        groups: adjacentConversationGroups(this.cells, targetQ, targetR, this.conversations),
      };
    } catch (error) {
      this.dataStale = true;
      this.setConnectionState(navigator.onLine ? "stale" : "offline");
      return { ok: false, code: "unavailable", choices: [], error };
    }
  }

  mergeCell(cell, options = {}) {
    const q = Number(cell && cell.q);
    const r = Number(cell && cell.r);
    if (!cell || typeof cell.id !== "string" || !validCoordinate(q, r)) return null;
    const normalized = {
      ...cell,
      q,
      r,
      seq: Number(cell.seq) || 0,
      version: Number(cell.version) || 1,
      contentVersion: Number(cell.contentVersion) || 1,
    };
    const previous = this.cellsById.get(normalized.id);
    if (previous
      && normalized.version === previous.version
      && normalized.contentVersion === previous.contentVersion) {
      normalized.viewerReported = Boolean(previous.viewerReported || normalized.viewerReported);
      normalized.viewerHidden = Boolean(previous.viewerHidden || normalized.viewerHidden);
      if (normalized.viewerHidden) normalized.body = null;
    }
    if (previous && (normalized.version < previous.version
      || (normalized.version === previous.version && normalized.contentVersion < previous.contentVersion))) return previous;
    const projectionFields = ["version", "contentVersion", "status", "body", "author", "membership", "replyToId", "conversationId", "viewerReported", "viewerHidden", "isMine"];
    const projectionChanged = !previous || projectionFields.some((field) => previous[field] !== normalized[field]);
    const contentVersionChanged = Boolean(previous && previous.contentVersion !== normalized.contentVersion);
    const canonical = previous || normalized;
    if (previous) Object.assign(canonical, normalized);
    this.cells.set(cellKey(q, r), canonical);
    this.cellsById.set(canonical.id, canonical);
    this.cellChunks.set(canonical.id, chunkFor(q, r).key);
    const listIndex = this.listCells.findIndex((item) => item.id === canonical.id);
    if (listIndex >= 0) this.listCells[listIndex] = canonical;
    if (this.readerCell?.id === canonical.id) {
      const reportForm = this.root.querySelector("[data-wall-report-form]");
      const operatorForm = this.root.querySelector("[data-wall-operator-form]");
      const reportWasOpen = Boolean(reportForm && !reportForm.hidden);
      const operatorWasOpen = Boolean(operatorForm && !operatorForm.hidden);
      const reportStillValid = reportWasOpen && !contentVersionChanged && canonical.status === "visible"
        && !canonical.isMine && !canonical.viewerReported;
      const operatorStillValid = operatorWasOpen && !contentVersionChanged && canonical.status === "visible" && this.operator;
      const preserveForms = this.readerBusy || reportStillValid || operatorStillValid;
      const formInvalidated = (reportWasOpen || operatorWasOpen) && !preserveForms;
      if (contentVersionChanged) this.reportRequest = null;
      this.readerCell = canonical;
      if (projectionChanged && (this.reader?.open || this.reader?.hasAttribute("open"))) {
        this.paintReaderCell(canonical, { preserveForms });
        if (formInvalidated) this.toast("That comb changed, so the open review form was closed. Review the current version before acting.");
      }
    }
    if (options.renderList !== false
      && projectionChanged && listIndex >= 0 && this.mode === "list" && this.mounted) this.renderList();
    return canonical;
  }

  pruneChunkCache() {
    const protectedIds = new Set([
      this.readerCell?.id,
      this.composeState?.cellId,
      this.composeState?.replyToId,
      ...this.listCells.map((cell) => cell.id),
    ].filter(Boolean));
    const candidates = [...this.loadedChunks]
      .filter((key) => !this.visibleChunkKeys.has(key) && !this.inflightChunks.has(key))
      .sort((left, right) => (this.chunkAccess.get(left) || 0) - (this.chunkAccess.get(right) || 0));
    while ((this.loadedChunks.size > MAX_CACHED_CHUNKS || this.cellsById.size > MAX_CACHED_CELLS) && candidates.length) {
      const key = candidates.shift();
      this.loadedChunks.delete(key);
      this.chunkAccess.delete(key);
      for (const [cellId, cellChunk] of this.cellChunks) {
        if (cellChunk !== key || protectedIds.has(cellId)) continue;
        const cell = this.cellsById.get(cellId);
        if (cell && this.cells.get(cellKey(cell.q, cell.r))?.id === cellId) this.cells.delete(cellKey(cell.q, cell.r));
        this.cellsById.delete(cellId);
        this.cellChunks.delete(cellId);
      }
    }
    if (this.conversations.size > 600) {
      const active = new Set([...this.cellsById.values()].map((cell) => cell.conversationId));
      for (const id of this.conversations.keys()) {
        if (this.conversations.size <= 600) break;
        if (!active.has(id) && id !== this.listConversationId) this.conversations.delete(id);
      }
    }
  }

  scheduleViewport() {
    if (!this.active || this.renderFrame) return;
    this.renderFrame = requestAnimationFrame(() => {
      this.renderFrame = null;
      void this.refreshVisible(false);
    });
  }

  queueRealtimeEvent(event) {
    const revision = Number(event?.revision);
    const q = Number(event?.q);
    const r = Number(event?.r);
    const cellId = typeof event?.cellId === "string" ? event.cellId : "";
    if (!Number.isSafeInteger(revision) || revision < 1 || !cellId || !validCoordinate(q, r)) return;
    const eventKey = `${cellId}:${revision}`;
    if (this.seenRealtimeEvents.has(eventKey)) return;
    this.seenRealtimeEvents.add(eventKey);
    if (this.seenRealtimeEvents.size > 512) this.seenRealtimeEvents.delete(this.seenRealtimeEvents.values().next().value);
    const sanitized = { ...event, cellId, revision, q, r, eventKey };
    if (this.pendingEvents.size >= 128 && !this.pendingEvents.has(event.cellId)) {
      const oldest = this.pendingEvents.keys().next().value;
      if (oldest) {
        const dropped = this.pendingEvents.get(oldest);
        this.pendingEvents.delete(oldest);
        if (dropped?.eventKey) this.seenRealtimeEvents.delete(dropped.eventKey);
        if (validCoordinate(Number(dropped?.q), Number(dropped?.r))) {
          const droppedChunk = chunkFor(Number(dropped.q), Number(dropped.r));
          this.loadedChunks.delete(droppedChunk.key);
          if (this.inflightChunks.has(droppedChunk.key)) this.dirtyChunks.add(droppedChunk.key);
        }
      }
    }
    const prior = this.pendingEvents.get(cellId);
    if (!prior || revision >= prior.revision) this.pendingEvents.set(cellId, sanitized);
    if (this.eventTimer) return;
    this.eventTimer = window.setTimeout(() => {
      this.eventTimer = null;
      const events = [...this.pendingEvents.values()];
      this.pendingEvents.clear();
      const drain = typeof this.queueRealtimeDrain === "function"
        ? this.queueRealtimeDrain
        : HiveWallController.prototype.queueRealtimeDrain;
      void drain.call(this, events);
    }, 140);
  }

  queueRealtimeDrain(events) {
    if (!this.realtimeDrainQueue) this.realtimeDrainQueue = new Map();
    for (const event of Array.isArray(events) ? events : []) {
      if (!event?.eventKey || !event?.cellId) continue;
      const prior = this.realtimeDrainQueue.get(event.cellId);
      if (!prior || Number(event.revision) >= Number(prior.revision)) {
        this.realtimeDrainQueue.set(event.cellId, event);
      }
    }
    if (this.realtimeDrainPromise || !this.realtimeDrainQueue.size) return this.realtimeDrainPromise;
    const activationGeneration = this.activationGeneration;
    const identityGeneration = this.identityGeneration;
    const run = async () => {
      while (this.realtimeDrainQueue.size
        && this.isContextCurrent(activationGeneration, identityGeneration)) {
        const batch = [...this.realtimeDrainQueue.values()];
        this.realtimeDrainQueue.clear();
        for (const event of batch) this.consumingEvents.set(event.eventKey, event);
        await this.consumeRealtimeEvents(batch);
      }
    };
    let request;
    request = run().finally(() => {
      if (this.realtimeDrainPromise === request) this.realtimeDrainPromise = null;
      if (this.realtimeDrainQueue.size && this.active) this.queueRealtimeDrain([]);
    });
    this.realtimeDrainPromise = request;
    return request;
  }

  scheduleRealtimeRetries(activationGeneration, identityGeneration) {
    if (!this.retryEvents.size || this.eventRetryTimer) return;
    this.eventRetryTimer = window.setTimeout(() => {
      this.eventRetryTimer = null;
      if (this.isContextCurrent(activationGeneration, identityGeneration)) {
        const retryEvents = [...this.retryEvents.values()];
        this.retryEvents.clear();
        for (const event of retryEvents) this.queueRealtimeEvent(event);
      }
    }, 600);
  }

  scheduleListReconciliation(activationGeneration, identityGeneration, immediate = false) {
    if (this.listReconcileTimer || this.mode !== "list" || !this.listReconcileEvents.size
      || !this.isContextCurrent(activationGeneration, identityGeneration)) return;
    const loadedIds = new Set(this.listCells.map((cell) => cell?.id).filter(Boolean));
    const queued = [...this.listReconcileEvents.values()]
      .filter((event) => event.evicted || loadedIds.has(event.cellId))
      .slice(0, 64);
    if (!queued.length) return;
    const attempt = Math.max(...queued.map((event) => Number(event.reconcileAttempts) || 1));
    const delay = immediate ? 0 : Math.min(30_000, 600 * (2 ** Math.min(6, Math.max(0, attempt - 1))));
    this.listReconcileTimer = window.setTimeout(() => {
      this.listReconcileTimer = null;
      if (!this.isContextCurrent(activationGeneration, identityGeneration) || this.mode !== "list") return;
      for (const event of queued) {
        const current = this.listReconcileEvents.get(event.cellId);
        if (!current || current.revision !== event.revision) continue;
        this.seenRealtimeEvents.delete(event.eventKey);
        this.queueRealtimeEvent(event);
      }
    }, delay);
  }

  wakeListReconciliation() {
    if (!this.active || !this.mounted || this.mode !== "list" || !this.listReconcileEvents.size) return;
    if (this.listReconcileTimer) window.clearTimeout(this.listReconcileTimer);
    this.listReconcileTimer = null;
    this.scheduleListReconciliation(this.activationGeneration, this.identityGeneration, true);
  }

  async consumeRealtimeEvents(events) {
    const activationGeneration = this.activationGeneration;
    const identityGeneration = this.identityGeneration;
    const cacheGeneration = this.publicCacheGeneration;
    const realtimeMode = this.mode;
    const listRequestGeneration = this.listRequestGeneration;
    const listConversationId = this.listConversationId;
    const retainedListRevision = Number(this.listWindowRevision) || 0;
    const realtimeContextCurrent = () => (
      this.isContextCurrent(activationGeneration, identityGeneration)
      && cacheGeneration === this.publicCacheGeneration
      && realtimeMode === this.mode
      && (realtimeMode !== "list" || (
        listRequestGeneration === this.listRequestGeneration
        && listConversationId === this.listConversationId
      ))
    );
    const requeueForCurrentContext = () => {
      if (!this.isContextCurrent(activationGeneration, identityGeneration)) return;
      for (const event of events) {
        this.seenRealtimeEvents.delete(event.eventKey);
        this.queueRealtimeEvent(event);
      }
    };
    try {
      await this.refreshBounds();
      if (!this.isContextCurrent(activationGeneration, identityGeneration)) return;
      if (!realtimeContextCurrent()) {
        requeueForCurrentContext();
        return;
      }
      const authoritativeRevision = this.bounds.revision;
      const verified = events.filter((event) => (
        event.revision > 0 && event.revision <= authoritativeRevision
      ));
      for (const event of events) {
        if (event.revision > authoritativeRevision) {
          this.seenRealtimeEvents.delete(event.eventKey);
          if ((Number(event.retries) || 0) < 3) {
            const retried = { ...event, retries: (Number(event.retries) || 0) + 1 };
            const prior = this.retryEvents.get(event.cellId);
            if (!prior || retried.revision >= prior.revision) this.retryEvents.set(event.cellId, retried);
          }
        }
      }
      this.scheduleRealtimeRetries(activationGeneration, identityGeneration);
      if (!verified.length) return;
      const previouslyLoadedChunks = new Set();
      const previouslyInflightChunks = new Set();
      for (const event of verified) {
        const changedChunk = chunkFor(event.q, event.r);
        if (this.loadedChunks.has(changedChunk.key)) previouslyLoadedChunks.add(changedChunk.key);
        if (this.inflightChunks.has(changedChunk.key)) previouslyInflightChunks.add(changedChunk.key);
        this.loadedChunks.delete(changedChunk.key);
        if (this.inflightChunks.has(changedChunk.key)) this.dirtyChunks.add(changedChunk.key);
      }
      if (realtimeMode === "list") {
        const expectedRevisionCount = authoritativeRevision - retainedListRevision;
        const receivedRevisions = new Set(verified
          .map((event) => event.revision)
          .filter((revision) => revision > retainedListRevision && revision <= authoritativeRevision));
        const completeRevisionChain = expectedRevisionCount >= 0
          && receivedRevisions.size === expectedRevisionCount;
        if ((this.listCells.length || this.cells?.size || this.conversations?.size || this.readerCell)
          && !completeRevisionChain) {
          // A later event arrived after at least one missed invalidation. The
          // missing revision could belong to any older retained row, so clear
          // the rendered window before attempting a clean page-one rebuild.
          const rebuildView = this.captureAccessibleListView(true);
          this.invalidatePublicProjectionCache({ resetConversation: false });
          await this.refreshList(true, rebuildView);
          if (!this.isContextCurrent(activationGeneration, identityGeneration)
            || this.mode !== "list" || this.listConversationId !== listConversationId) return;
          this.announce("The Wall list was rebuilt after catching up with missed changes.");
          return;
        }
        // A newest-page refresh cannot authoritatively update a comb that the
        // reader loaded several pages ago. Re-read every changed comb already
        // present in the retained list window, merge those rows without
        // rendering, then let the single bounded list refresh reconcile and
        // paint the complete window while preserving focus and scroll.
        const loadedIds = new Set(this.listCells.map((cell) => cell?.id).filter(Boolean));
        const loadedEvents = verified.filter((event) => (
          loadedIds.has(event.cellId) || this.listReconcileEvents.get(event.cellId)?.evicted
        ));
        const reconcileView = this.captureAccessibleListView(true);
        // Cell and conversation caches also feed reply labels and participant
        // names. Evict every verified support projection, even when the
        // changed comb itself is outside the retained list window.
        for (const event of verified) {
          if (event.conversationId) this.conversations.delete(event.conversationId);
          this.evictPublicCellProjection(event.cellId);
        }
        this.renderList();
        this.restoreAccessibleListView(reconcileView, this.listRequestGeneration, this.listConversationId);
        if (loadedEvents.length) {
          for (const event of loadedEvents) {
            const prior = this.listReconcileEvents.get(event.cellId);
            if (!prior && this.listReconcileEvents.size >= MAX_LIST_RECONCILE_EVENTS) {
              // A long outage plus a very large retained history must not grow
              // an unbounded retry map. Drop the hidden/list DOM window; the
              // immediate page read below and later Load more calls rebuild it
              // authoritatively instead of ever showing a stale cached row.
              const rebuildView = this.captureAccessibleListView(true);
              this.invalidatePublicProjectionCache({ resetConversation: false });
              await this.refreshList(true, rebuildView);
              if (this.isContextCurrent(activationGeneration, identityGeneration)
                && this.mode === "list" && this.listConversationId === listConversationId) {
                this.announce("The Wall list was rebuilt after a large batch of changes.");
              }
              return;
            }
            if (!prior || event.revision >= prior.revision) {
              this.listReconcileEvents.set(event.cellId, {
                ...event,
                reconcileAttempts: Math.max(1, Number(prior?.reconcileAttempts) || Number(event.reconcileAttempts) || 1),
              });
            }
          }
          if (!realtimeContextCurrent()) {
            requeueForCurrentContext();
            return;
          }
          for (const event of loadedEvents) {
            const obligation = this.listReconcileEvents.get(event.cellId);
            if (obligation) this.listReconcileEvents.set(event.cellId, { ...obligation, evicted: true });
          }
          const authoritative = await Promise.allSettled(
            loadedEvents.map((event) => this.backend.wallCell(event.cellId)),
          );
          if (!realtimeContextCurrent()) {
            requeueForCurrentContext();
            return;
          }
          const advancedExact = authoritative.some((outcome) => (
            outcome.status === "fulfilled"
            && Number.isSafeInteger(Number(outcome.value?.bounds?.revision))
            && (
              Number(outcome.value.bounds.revision) > authoritativeRevision
              || this.viewerProjectionAdvanced(outcome.value.bounds)
            )
          ));
          if (advancedExact) {
            this.invalidatePublicProjectionCache({ resetConversation: false });
            await this.refreshList(true, reconcileView);
            if (this.isContextCurrent(activationGeneration, identityGeneration)
              && this.mode === "list" && this.listConversationId === listConversationId) {
              this.announce("The Wall list was rebuilt after a newer change arrived mid-refresh.");
            }
            return;
          }
          const changedCells = [];
          for (let index = 0; index < authoritative.length; index += 1) {
            const outcome = authoritative[index];
            const event = loadedEvents[index];
            const result = outcome.status === "fulfilled" ? outcome.value : null;
            if (!result?.cell || !result?.bounds
              || Number(result.bounds.revision) !== authoritativeRevision
              || !this.applyAuthoritativeBounds(result.bounds)) {
              this.seenRealtimeEvents.delete(event.eventKey);
              const prior = this.listReconcileEvents.get(event.cellId) || event;
              this.listReconcileEvents.set(event.cellId, {
                ...prior,
                reconcileAttempts: Math.min(20, (Number(prior.reconcileAttempts) || 1) + 1),
              });
              continue;
            }
            if (result?.conversation?.id) this.conversations.set(result.conversation.id, result.conversation);
            const canonical = this.mergeCell(result.cell, { renderList: false });
            if (canonical) changedCells.push(canonical);
            const obligation = this.listReconcileEvents.get(event.cellId);
            if (!obligation || obligation.revision <= event.revision) this.listReconcileEvents.delete(event.cellId);
          }
          this.listCells = reconcileAccessibleListWindow(this.listCells, changedCells, this.listConversationId);
          this.scheduleListReconciliation(activationGeneration, identityGeneration);
          if (completeRevisionChain) {
            // Failed exact reads are safe to account only because their public
            // rows were synchronously evicted above; the retained obligation
            // keeps retrying until the authoritative comb can be reinserted.
            this.listWindowRevision = Math.max(this.listWindowRevision, authoritativeRevision);
          }
        } else if (completeRevisionChain) {
          this.listWindowRevision = Math.max(this.listWindowRevision, authoritativeRevision);
        }
        await this.refreshList(true, reconcileView);
        if (!this.isContextCurrent(activationGeneration, identityGeneration)
          || this.mode !== "list" || this.listConversationId !== listConversationId) return;
        this.scheduleListReconciliation(activationGeneration, identityGeneration);
        this.announce(`${verified.length} wall ${verified.length === 1 ? "comb has" : "combs have"} changed.`);
        return;
      }
      const retainedWallRevision = Number(this.wallWindowRevision) || 0;
      const expectedRevisionCount = authoritativeRevision - retainedWallRevision;
      const receivedRevisions = new Set(verified
        .map((event) => event.revision)
        .filter((revision) => revision > retainedWallRevision && revision <= authoritativeRevision));
      const completeRevisionChain = expectedRevisionCount >= 0
        && receivedRevisions.size === expectedRevisionCount;
      if (this.hasPublicProjection("wall") && !completeRevisionChain) {
        this.invalidatePublicProjectionCache({ resetConversation: false });
        await this.refreshVisible(true);
        if (this.isContextCurrent(activationGeneration, identityGeneration) && this.mode === "wall") {
          this.announce("The Wall was rebuilt after catching up with missed changes.");
        }
        return;
      }
      const newEvents = verified.filter((event) => event.revision > retainedWallRevision);
      if (!newEvents.length) return;
      if (!this.hasPublicProjection("wall")) {
        await this.refreshVisible(true);
        return;
      }
      const affectedChunks = new Map();
      const visibleChunks = this.chunksForVisibleRegion();
      let readerAffected = false;
      for (const event of newEvents) {
        const changedChunk = chunkFor(event.q, event.r);
        affectedChunks.set(changedChunk.key, changedChunk);
        if (event.conversationId) {
          this.conversations.delete(event.conversationId);
          if (this.readerCell?.conversationId === event.conversationId) readerAffected = true;
        }
      }
      if (readerAffected) this.closeReaderForProjectionRefresh();
      if ([...affectedChunks.keys()].some((key) => previouslyInflightChunks.has(key))) {
        this.invalidatePublicProjectionCache({ resetConversation: false });
        await this.refreshVisible(true);
        return;
      }
      const reloadChunks = [];
      for (const [key, chunk] of affectedChunks) {
        const hadCachedCells = [...this.cellChunks.values()].some((cellChunk) => cellChunk === key);
        this.evictWallChunkProjection(key, { preserveGeometry: true });
        if (previouslyLoadedChunks.has(key) || hadCachedCells || visibleChunks.has(key)) reloadChunks.push(chunk);
      }
      // Blank only the changed words while retaining inert comb geometry until
      // the authoritative region arrives. This avoids a board-wide or chunk-wide
      // flash without ever displaying changed text from the prior revision.
      const reconciled = await Promise.all(reloadChunks
        .map((chunk) => this.loadChunk(chunk, true, authoritativeRevision)));
      if (!realtimeContextCurrent()) {
        if (this.isContextCurrent(activationGeneration, identityGeneration)) await this.refreshVisible(true);
        return;
      }
      if (!reconciled.every((result) => result?.ok && Number(result.revision) === authoritativeRevision)) {
        this.invalidatePublicProjectionCache({ resetConversation: false });
        await this.refreshVisible(true);
        return;
      }
      this.wallWindowRevision = Math.max(retainedWallRevision, authoritativeRevision);
      this.wall?.setAttribute?.("aria-busy", "false");
      this.render();
      this.announce(`${verified.length} wall ${verified.length === 1 ? "comb has" : "combs have"} changed.`);
    } finally {
      for (const event of events) this.consumingEvents.delete(event.eventKey);
    }
  }

  render() {
    if (!this.mounted || this.mode !== "wall") return;
    const visible = this.visibleCoordinates();
    const keep = new Set();
    for (const [key, cell] of this.cells) {
      if (cell.q < visible.qMin || cell.q > visible.qMax || cell.r < visible.rMin || cell.r > visible.rMax) continue;
      if (hexDistance(cell.q, cell.r) > this.bounds.radius) continue;
      keep.add(key);
      let node = this.nodes.get(key);
      if (!node) {
        node = document.createElement("button");
        node.type = "button";
        node.className = "honey-comb";
        node.dataset.cellKey = key;
        node.innerHTML = '<span class="honey-comb-inner"><span class="honey-comb-author"></span><span class="honey-comb-body"></span><span class="honey-comb-foot"></span></span>';
        this.cellLayer.append(node);
        this.nodes.set(key, node);
      }
      this.paintCellNode(node, cell);
    }
    for (const [key, node] of this.nodes) {
      if (!keep.has(key)) {
        node.remove();
        this.nodes.delete(key);
      }
    }
    this.renderConnections(keep);
    if (this.growthIntent) this.renderGrowthSlots();
    this.updateTarget();
    this.updateNewCount();
  }

  paintCellNode(node, cell) {
    const point = axialToPixel(cell.q, cell.r, this.hexRadius);
    node.style.left = `calc(var(--honey-origin-x) + ${point.x}px)`;
    node.style.top = `calc(var(--honey-origin-y) + ${point.y}px)`;
    node.style.setProperty("--thread-hue", String(hueFor(cell.conversationId)));
    node.dataset.threadPattern = String(patternFor(cell.conversationId));
    node.dataset.status = cell.status;
    node.dataset.conversationId = cell.conversationId;
    node.id = `honey-cell-${cell.id}`;
    node.tabIndex = -1;
    node.disabled = false;
    delete node.dataset.placeholder;
    node.classList.remove("is-refreshing");
    node.classList.toggle("is-branch-origin", cell.origin?.kind === "branch");
    node.classList.toggle("is-meet-origin", cell.origin?.kind === "meet");
    node.classList.toggle("is-unseen", !this.seen.has(cell.id));
    node.classList.toggle("is-mine", Boolean(cell.isMine));
    node.classList.toggle("is-viewer-hidden", Boolean(cell.viewerHidden));
    const author = node.querySelector(".honey-comb-author");
    const body = node.querySelector(".honey-comb-body");
    const foot = node.querySelector(".honey-comb-foot");
    setTextIfChanged(author, cell.author || (cell.status === "quarantined" ? "Under review" : "Open space kept"));
    setTextIfChanged(body, statusText(cell));
    setTextIfChanged(foot, `${timeLabel(cell.createdAt)} · ${cell.membership === "alumni" ? "DHG" : cell.membership === "guest" ? "Guest" : "Hive"}`);
    const ariaLabel = `${author.textContent}: ${statusText(cell)}`;
    if (node.getAttribute("aria-label") !== ariaLabel) node.setAttribute("aria-label", ariaLabel);
  }

  renderConnections(visibleKeys) {
    const keep = new Set();
    const paintLine = (key, fromCell, toCell, relation = "growth") => {
      const from = axialToPixel(fromCell.q, fromCell.r, this.hexRadius);
      const to = axialToPixel(toCell.q, toCell.r, this.hexRadius);
      const dx = to.x - from.x;
      const dy = to.y - from.y;
      let line = this.connectionNodes.get(key);
      if (!line) {
        line = document.createElement("span");
        line.className = "honey-link";
        this.connectionLayer.append(line);
        this.connectionNodes.set(key, line);
      }
      keep.add(key);
      line.classList.remove("is-refreshing");
      line.dataset.relation = relation;
      line.style.left = `calc(var(--honey-origin-x) + ${from.x}px)`;
      line.style.top = `calc(var(--honey-origin-y) + ${from.y}px)`;
      line.style.width = `${Math.hypot(dx, dy)}px`;
      line.style.transform = `rotate(${Math.atan2(dy, dx)}rad)`;
      line.style.setProperty("--thread-hue", String(hueFor(toCell.conversationId)));
      line.style.setProperty("--source-hue", String(hueFor(fromCell.conversationId)));
    };
    for (const key of visibleKeys) {
      const cell = this.cells.get(key);
      if (!cell) continue;
      if (cell.growthParentId) {
        const parent = this.cellsById.get(cell.growthParentId);
        if (parent && visibleKeys.has(cellKey(parent.q, parent.r))) {
          paintLine(`growth:${cell.id}:${parent.id}`, parent, cell, "growth");
        }
      }
      for (const anchorLink of cell.origin?.anchors || []) {
        const anchor = this.cellsById.get(anchorLink.cellId);
        if (!anchor || !visibleKeys.has(cellKey(anchor.q, anchor.r))) continue;
        paintLine(`origin:${cell.id}:${anchor.id}`, anchor, cell, cell.origin.kind);
      }
    }
    for (const [key, line] of this.connectionNodes) {
      if (keep.has(key)) continue;
      line.remove();
      this.connectionNodes.delete(key);
    }
  }

  updateTarget() {
    if (!this.target) return;
    if (this.selectedCellKey) {
      const previous = this.nodes.get(this.selectedCellKey);
      previous?.classList.remove("is-selected");
      previous?.removeAttribute("aria-current");
    }
    const point = axialToPixel(this.selected.q, this.selected.r, this.hexRadius);
    this.target.style.left = `calc(var(--honey-origin-x) + ${point.x}px)`;
    this.target.style.top = `calc(var(--honey-origin-y) + ${point.y}px)`;
    const occupied = this.cells.get(cellKey(this.selected.q, this.selected.r));
    this.target.classList.toggle("is-occupied", Boolean(occupied));
    this.target.querySelector("span").textContent = occupied ? "•" : "+";
    if (occupied) {
      this.selectedCellKey = cellKey(occupied.q, occupied.r);
      const selectedNode = this.nodes.get(this.selectedCellKey);
      if (selectedNode) {
        selectedNode.classList.add("is-selected");
        selectedNode.setAttribute("aria-current", "true");
        this.stage?.setAttribute("aria-activedescendant", selectedNode.id || `honey-cell-${occupied.id}`);
      } else {
        this.stage?.removeAttribute("aria-activedescendant");
      }
    } else {
      this.selectedCellKey = null;
      this.stage?.removeAttribute("aria-activedescendant");
    }
  }

  handleClick(event) {
    const action = event.target.closest("[data-wall-action]");
    if (action) {
      this.runToolbarAction(action.dataset.wallAction);
      return;
    }
    const closer = event.target.closest("[data-wall-close]");
    if (closer) {
      const dialog = closer.dataset.wallClose === "compose" ? this.compose
        : closer.dataset.wallClose === "reader" ? this.reader
          : this.watch;
      this.closeDialog(dialog);
      return;
    }
    const watchCase = event.target.closest("[data-wall-watch-case]");
    if (watchCase) {
      if (this.watchBusy) return;
      void this.openWatchCase(watchCase.dataset.wallWatchCase, Number(watchCase.dataset.wallWatchVersion));
      return;
    }
    if (event.target.closest("[data-wall-watch-more]")) {
      if (!this.watchBusy) void this.loadWatchQueue(false, this.watchGeneration);
      return;
    }
    const watchAction = event.target.closest("[data-wall-watch-action]");
    if (watchAction) {
      void this.moderateWatchCase(watchAction.dataset.wallWatchAction);
      return;
    }
    const readerAction = event.target.closest("[data-wall-reader-action]");
    if (readerAction) {
      if (this.readerBusy) return;
      void this.runReaderAction(readerAction.dataset.wallReaderAction);
      return;
    }
    const listCell = event.target.closest("[data-wall-list-cell]");
    if (listCell) {
      void this.openListCell(listCell.dataset.wallListCell, listCell);
      return;
    }
    const listConversation = event.target.closest("[data-wall-list-conversation]");
    if (listConversation) {
      void this.openListConversation(listConversation.dataset.wallListConversation);
      return;
    }
    const growthSlot = event.target.closest("[data-wall-growth-key]");
    if (growthSlot) {
      const [q, r] = String(growthSlot.dataset.wallGrowthKey || "").split(":").map(Number);
      if (validCoordinate(q, r, this.bounds.radius)) this.chooseGrowthCoordinate({ q, r }, growthSlot);
      return;
    }
    const cellNode = event.target.closest("[data-cell-key]");
    if (cellNode) {
      if (Date.now() <= this.suppressCellClickUntil) {
        event.preventDefault();
        event.stopPropagation();
        this.suppressCellClickUntil = 0;
        return;
      }
      const cell = this.cells.get(cellNode.dataset.cellKey);
      if (cell) {
        this.selected = { q: cell.q, r: cell.r };
        this.updateTarget();
        this.openReader(cell);
      }
    }
  }

  runToolbarAction(action) {
    if (action === "add") {
      if (this.bounds.readOnly) {
        this.toast("The Wall is open to read while Hive Watch is verified. New combs will open soon.");
        return;
      }
      const coordinate = this.cells.has(cellKey(this.selected.q, this.selected.r))
        ? this.findNearestOpen(this.selected)
        : this.selected;
      if (!coordinate) {
        this.toast("No open comb is loaded yet. Center the Wall or try again after it grows.");
        return;
      }
      this.selected = coordinate;
      this.updateTarget();
      this.ensureCoordinateVisible(coordinate.q, coordinate.r);
      void this.preparePlacement(coordinate);
    }
    else if (action === "new") this.findNew();
    else if (action === "mine") this.findMine();
    else if (action === "center") this.centerWall(true);
    else if (action === "fit") this.fitWall(true);
    else if (action === "zoom-in") {
      this.applyCameraMotion("smooth");
      this.setZoom(this.zoom * ZOOM_STEP, null, { announce: true });
    }
    else if (action === "zoom-out") {
      this.applyCameraMotion("smooth");
      this.setZoom(this.zoom / ZOOM_STEP, null, { announce: true });
    }
    else if (action === "mode") this.setMode(this.mode === "wall" ? "list" : "wall");
    else if (action === "resume") void this.resumeCurrentDraft();
    else if (action === "load-more") void this.refreshList(false);
    else if (action === "list-all") void this.showAllListMessages();
    else if (action === "watch") void this.openWatch();
    else if (action === "cancel-growth") this.cancelGrowthIntent();
    else if (action === "nearest-growth") this.moveGrowthToNearestFrontier();
  }

  handleStageKey(event) {
    if (isEditableField(event.target)) return;
    // Shift with an arrow glides the camera without moving the viewer's place
    // in the wall, so looking around never loses the selected comb.
    if (event.shiftKey && ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(event.key)) {
      event.preventDefault();
      const step = PAN_KEY_STEP;
      const dx = event.key === "ArrowRight" ? step : event.key === "ArrowLeft" ? -step : 0;
      const dy = event.key === "ArrowDown" ? step : event.key === "ArrowUp" ? -step : 0;
      this.stopMomentum();
      this.panBy(dx, dy);
      return;
    }
    if (event.key === "+" || event.key === "=") {
      event.preventDefault();
      this.applyCameraMotion("smooth");
      this.setZoom(this.zoom * ZOOM_STEP, null, { announce: true });
      return;
    }
    if (event.key === "-" || event.key === "_") {
      event.preventDefault();
      this.applyCameraMotion("smooth");
      this.setZoom(this.zoom / ZOOM_STEP, null, { announce: true });
      return;
    }
    if (event.key === "0") {
      event.preventDefault();
      this.applyCameraMotion("smooth");
      this.fitWall(true);
      return;
    }
    let direction = null;
    if (event.key === "ArrowRight") direction = 0;
    else if (event.key === "ArrowUp") direction = 2;
    else if (event.key === "ArrowLeft") direction = 3;
    else if (event.key === "ArrowDown") direction = 5;
    else if (["e", "9"].includes(event.key.toLowerCase())) direction = 1;
    else if (["q", "7"].includes(event.key.toLowerCase())) direction = 2;
    else if (["a", "4"].includes(event.key.toLowerCase())) direction = 3;
    else if (["z", "1"].includes(event.key.toLowerCase())) direction = 4;
    else if (["c", "3"].includes(event.key.toLowerCase())) direction = 5;
    else if (["d", "6"].includes(event.key.toLowerCase())) direction = 0;
    if (event.key === "Escape" && this.growthIntent) {
      event.preventDefault();
      this.cancelGrowthIntent();
      return;
    }
    if (direction != null) {
      event.preventDefault();
      const [dq, dr] = HEX_DIRECTIONS[direction];
      const origin = this.growthIntent?.origin || this.selected;
      const next = { q: origin.q + dq, r: origin.r + dr };
      if (validCoordinate(next.q, next.r, this.bounds.radius)) {
        this.selected = next;
        this.updateTarget();
        this.ensureCoordinateVisible(next.q, next.r);
        const growthSlot = this.growthSlots?.querySelector?.(`[data-wall-growth-key="${cellKey(next.q, next.r)}"]`);
        if (this.growthIntent && growthSlot && !growthSlot.disabled) growthSlot.focus({ preventScroll: true });
        this.announce(`${HEX_DIRECTION_LABELS[direction]} comb${this.cells.has(cellKey(next.q, next.r)) ? ", occupied" : ", open"}.`);
      }
    } else if (event.key === "Home") {
      event.preventDefault();
      this.selected = { q: 0, r: 0 };
      this.updateTarget();
      this.ensureCoordinateVisible(0, 0);
      this.announce("Centered on comb 0, 0.");
    } else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      if (this.growthIntent && this.isGrowthCoordinate(this.selected)) {
        this.chooseGrowthCoordinate(this.selected, event.target);
        return;
      }
      const cell = this.cells.get(cellKey(this.selected.q, this.selected.r));
      if (cell) this.openReader(cell);
      else void this.preparePlacement(this.selected);
    }
  }

  handlePointerDown(event) {
    if (event.button !== 0 || event.target.closest("[data-wall-growth-key]")) return;
    this.stopMomentum();
    this.plane?.classList.remove("is-gliding");
    this.activePointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
    if (this.activePointers.size === 2) {
      this.beginPinch();
      this.pointer = null;
      return;
    }
    if (this.activePointers.size > 2) return;
    const cellNode = event.target.closest("[data-cell-key]");
    this.pointer = {
      id: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      lastX: event.clientX,
      lastY: event.clientY,
      lastAt: Date.now(),
      velocityX: 0,
      velocityY: 0,
      cameraX: this.cameraX,
      cameraY: this.cameraY,
      moved: false,
      startedOnCell: Boolean(cellNode),
      startedCellKey: cellNode?.dataset?.cellKey || null,
    };
    this.stage.setPointerCapture?.(event.pointerId);
  }

  beginPinch() {
    const points = [...this.activePointers.values()];
    if (points.length < 2) return;
    const rect = this.stage.getBoundingClientRect();
    const midX = (points[0].x + points[1].x) / 2 - rect.left;
    const midY = (points[0].y + points[1].y) / 2 - rect.top;
    this.plane?.classList.add("is-panning");
    this.pinch = {
      distance: Math.max(1, Math.hypot(points[0].x - points[1].x, points[0].y - points[1].y)),
      zoom: this.zoom,
      midX,
      midY,
      // Anchor the world point under the fingers so a pinch that also drifts
      // across the screen pans as well as zooms, the way a map does.
      world: this.stageToWorld(midX, midY),
      moved: false,
    };
  }

  updatePinch() {
    const points = [...this.activePointers.values()];
    if (!this.pinch || points.length < 2) return;
    const rect = this.stage.getBoundingClientRect();
    const midX = (points[0].x + points[1].x) / 2 - rect.left;
    const midY = (points[0].y + points[1].y) / 2 - rect.top;
    const distance = Math.max(1, Math.hypot(points[0].x - points[1].x, points[0].y - points[1].y));
    const ratio = distance / this.pinch.distance;
    if (Math.abs(ratio - 1) > 0.01 || Math.hypot(midX - this.pinch.midX, midY - this.pinch.midY) > 4) {
      this.pinch.moved = true;
    }
    const size = this.stageSize();
    this.zoom = this.clampZoom(this.pinch.zoom * ratio);
    this.cameraX = midX - size.width / 2 - this.pinch.world.x * this.zoom;
    this.cameraY = midY - (this.world.originY + this.pinch.world.y) * this.zoom;
    this.clampCamera();
    this.updatePlaneTransform();
    this.scheduleViewport();
  }

  handlePointerMove(event) {
    if (this.activePointers.has(event.pointerId)) {
      this.activePointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
    }
    if (this.pinch) {
      event.preventDefault();
      this.updatePinch();
      return;
    }
    if (!this.pointer || this.pointer.id !== event.pointerId) {
      if (event.pointerType === "mouse") this.selectFromPoint(event.clientX, event.clientY, false);
      return;
    }
    const dx = event.clientX - this.pointer.startX;
    const dy = event.clientY - this.pointer.startY;
    if (!this.pointer.moved && Math.hypot(dx, dy) > 8) this.pointer.moved = true;
    if (!this.pointer.moved) return;
    event.preventDefault();
    const now = Date.now();
    const elapsed = Math.max(1, now - this.pointer.lastAt);
    this.pointer.velocityX = (event.clientX - this.pointer.lastX) / elapsed;
    this.pointer.velocityY = (event.clientY - this.pointer.lastY) / elapsed;
    this.pointer.lastX = event.clientX;
    this.pointer.lastY = event.clientY;
    this.pointer.lastAt = now;
    // Drag moves the wall in both axes now. The old build only honoured
    // horizontal drags because vertical belonged to the document scroller.
    this.plane.classList.add("is-panning");
    this.cameraX = this.pointer.cameraX + dx;
    this.cameraY = this.pointer.cameraY + dy;
    this.clampCamera();
    this.updatePlaneTransform();
    this.updateTarget();
    this.scheduleViewport();
  }

  handlePointerUp(event) {
    this.activePointers.delete(event.pointerId);
    if (this.pinch) {
      if (this.activePointers.size < 2) {
        const moved = this.pinch.moved;
        this.pinch = null;
        this.plane?.classList.remove("is-panning");
        if (moved) this.suppressCellClickUntil = Date.now() + 420;
        // A finger lifted mid-pinch must not become a fresh one-finger drag
        // anchored to a stale origin, so re-seat whatever is still down.
        const [remaining] = [...this.activePointers.keys()];
        if (remaining != null) {
          const point = this.activePointers.get(remaining);
          this.pointer = {
            id: remaining,
            startX: point.x,
            startY: point.y,
            lastX: point.x,
            lastY: point.y,
            lastAt: Date.now(),
            velocityX: 0,
            velocityY: 0,
            cameraX: this.cameraX,
            cameraY: this.cameraY,
            moved: true,
            startedOnCell: false,
            startedCellKey: null,
          };
        }
      }
      return;
    }
    if (!this.pointer || this.pointer.id !== event.pointerId) return;
    const moved = this.pointer.moved;
    const startedOnCell = this.pointer.startedOnCell;
    const startedCellKey = this.pointer.startedCellKey;
    const velocityX = this.pointer.velocityX;
    const velocityY = this.pointer.velocityY;
    this.pointer = null;
    this.plane.classList.remove("is-panning");
    if (moved) {
      if (startedOnCell) this.suppressCellClickUntil = Date.now() + 420;
      this.startMomentum(velocityX, velocityY);
      return;
    }
    if (startedOnCell && startedCellKey) {
      const cell = this.cells.get(startedCellKey);
      if (cell) {
        this.selected = { q: cell.q, r: cell.r };
        this.updateTarget();
        // Pointer capture retargets the synthetic click to the stage in some
        // browsers. Open the original comb here, then suppress a duplicate
        // cell click in browsers that preserve the original target.
        this.suppressCellClickUntil = Date.now() + 420;
        // Touch browsers can retarget their compatibility click to the modal
        // backdrop because the reader opens during pointerup. Ignore only that
        // immediate backdrop click; subsequent taps still dismiss normally.
        if (event.pointerType !== "mouse") {
          this.suppressBackdropClick = { dialog: this.reader, until: Date.now() + 420 };
        }
        this.openReader(cell, this.nodes.get(startedCellKey));
      }
      return;
    }
    if (!startedOnCell) {
      const coordinate = this.selectFromPoint(event.clientX, event.clientY, true);
      if (!coordinate) return;
      const cell = this.cells.get(cellKey(coordinate.q, coordinate.r));
      if (cell) this.openReader(cell);
      else void this.preparePlacement(coordinate);
    }
  }

  cancelPointerGesture(event = null) {
    if (event?.pointerId != null) this.activePointers.delete(event.pointerId);
    else this.activePointers.clear();
    if (this.pointer?.moved && this.pointer.startedOnCell) this.suppressCellClickUntil = Date.now() + 420;
    this.pointer = null;
    this.pinch = null;
    this.plane?.classList.remove("is-panning");
  }

  // A flick keeps travelling briefly so crossing a wide wall does not need a
  // dozen drags. Reduced motion gets the exact drag distance and nothing more.
  startMomentum(velocityX, velocityY) {
    this.stopMomentum();
    if (this.motionBehavior("smooth") !== "smooth") return;
    let vx = Number(velocityX) || 0;
    let vy = Number(velocityY) || 0;
    if (Math.hypot(vx, vy) < 0.35) return;
    const step = () => {
      this.momentumFrame = null;
      vx *= 0.92;
      vy *= 0.92;
      if (Math.hypot(vx, vy) < 0.02) {
        this.scheduleViewport();
        return;
      }
      const beforeX = this.cameraX;
      const beforeY = this.cameraY;
      this.cameraX += vx * 16;
      this.cameraY += vy * 16;
      this.clampCamera();
      this.updatePlaneTransform();
      if (this.cameraX === beforeX && this.cameraY === beforeY) {
        this.scheduleViewport();
        return;
      }
      this.scheduleViewport();
      this.momentumFrame = requestAnimationFrame(step);
    };
    this.momentumFrame = requestAnimationFrame(step);
  }

  stopMomentum() {
    if (this.momentumFrame) cancelAnimationFrame(this.momentumFrame);
    this.momentumFrame = null;
  }

  // Plain wheel and two-finger trackpad scroll pan the wall. Ctrl or Command
  // wheel zooms at the cursor, which is also what a trackpad pinch sends.
  handleWheel(event) {
    if (this.mode !== "wall" || this.stage?.hidden) return;
    event.preventDefault();
    this.stopMomentum();
    const rect = this.stage.getBoundingClientRect();
    if (event.ctrlKey || event.metaKey) {
      const factor = Math.exp(-event.deltaY * 0.0022);
      this.setZoom(this.zoom * factor, { x: event.clientX - rect.left, y: event.clientY - rect.top });
      return;
    }
    const scale = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? rect.height : 1;
    this.panBy(event.deltaX * scale, event.deltaY * scale);
  }

  // The stage clips, so a comb outside the camera can still hold DOM focus
  // while being invisible. Every focus target inside the plane pulls the
  // camera to itself, which keeps keyboard and screen-reader focus on screen
  // now that the browser can no longer scroll the wall for us.
  handleStageFocus(event) {
    if (this.mode !== "wall" || !this.plane) return;
    const node = event.target?.closest?.("[data-cell-key], [data-wall-growth-key]");
    if (!node) return;
    const key = node.dataset?.cellKey || node.dataset?.wallGrowthKey;
    const [q, r] = String(key || "").split(":").map(Number);
    if (!Number.isFinite(q) || !Number.isFinite(r)) return;
    this.ensureCoordinateVisible(q, r, "auto");
  }

  handleDoubleClick(event) {
    if (this.mode !== "wall" || event.target.closest("[data-wall-growth-key]")) return;
    event.preventDefault();
    const rect = this.stage.getBoundingClientRect();
    const anchor = { x: event.clientX - rect.left, y: event.clientY - rect.top };
    // Double-click toggles between a readable close-up and the whole wall.
    const target = this.zoom > this.zoomFloor() * 1.05 ? this.fitZoom() : 1;
    this.applyCameraMotion("smooth");
    this.setZoom(target, anchor, { announce: true });
  }

  selectFromPoint(clientX, clientY, commit) {
    const stageRect = this.stage.getBoundingClientRect();
    const point = this.stageToWorld(clientX - stageRect.left, clientY - stageRect.top);
    const coordinate = pixelToAxial(point.x, point.y, this.hexRadius);
    if (!validCoordinate(coordinate.q, coordinate.r, this.bounds.radius)) return null;
    this.selected = coordinate;
    this.updateTarget();
    if (commit) this.stage.focus({ preventScroll: true });
    return coordinate;
  }

  growthCoordinates(origin = this.growthIntent?.origin) {
    if (!origin) return [];
    return neighbors(origin.q, origin.r).map((coordinate, direction) => ({
      ...coordinate,
      direction,
      directionLabel: HEX_DIRECTION_LABELS[direction],
      available: validCoordinate(coordinate.q, coordinate.r, this.bounds.radius)
        && !this.cells.has(cellKey(coordinate.q, coordinate.r)),
    }));
  }

  isGrowthCoordinate(coordinate) {
    return this.growthCoordinates().some((slot) => (
      slot.available && slot.q === Number(coordinate?.q) && slot.r === Number(coordinate?.r)
    ));
  }

  renderGrowthSlots() {
    if (!this.growthSlots || !this.growthTray) return;
    const state = this.growthIntent;
    if (!state) {
      if (this.growthFitFrame) cancelAnimationFrame(this.growthFitFrame);
      this.growthFitFrame = null;
      this.growthSlots.replaceChildren();
      this.growthTray.hidden = true;
      return;
    }
    const slots = this.growthCoordinates(state.origin);
    const available = slots.filter((slot) => slot.available);
    const retained = new Set();
    for (const slot of slots) {
      const key = cellKey(slot.q, slot.r);
      retained.add(key);
      let button = this.growthSlots.querySelector(`[data-wall-growth-key="${key}"]`);
      if (!button) {
        button = document.createElement("button");
        button.type = "button";
        button.className = "honey-growth-slot";
        button.dataset.wallGrowthKey = key;
        this.growthSlots.append(button);
      }
      button.dataset.direction = String(slot.direction);
      button.disabled = !slot.available;
      const point = axialToPixel(slot.q, slot.r, this.hexRadius);
      button.style.left = `calc(var(--honey-origin-x) + ${point.x}px)`;
      button.style.top = `calc(var(--honey-origin-y) + ${point.y}px)`;
      button.textContent = slot.available ? "+" : "×";
      button.setAttribute("aria-label", slot.available
        ? `${state.kind === "branch" ? "Start a branch" : "Reply"} ${slot.directionLabel}`
        : `${slot.directionLabel} is unavailable`);
    }
    for (const button of this.growthSlots.querySelectorAll("[data-wall-growth-key]")) {
      if (!retained.has(button.dataset.wallGrowthKey)) button.remove();
    }
    const originName = state.origin.author || "this comb";
    const copy = available.length
      ? `${available.length} open ${available.length === 1 ? "place" : "places"} around ${originName}. Choose a glowing comb; Escape cancels.`
      : `${originName} is surrounded. You can continue from the conversation's nearest open edge, or cancel.`;
    setTextIfChanged(this.root.querySelector("[data-wall-growth-copy]"), copy);
    const nearest = this.root.querySelector('[data-wall-action="nearest-growth"]');
    nearest.hidden = available.length > 0;
    nearest.textContent = state.kind === "branch" ? "Branch at nearest edge" : "Continue at nearest edge";
    this.growthTray.hidden = false;
    if (this.growthFitFrame) cancelAnimationFrame(this.growthFitFrame);
    this.growthFitFrame = requestAnimationFrame(() => {
      this.growthFitFrame = null;
      this.fitGrowthIntent();
    });
    const active = globalThis.document?.activeElement;
    if (active && this.growthSlots.contains(active) && active.disabled) {
      const fallback = this.growthSlots.querySelector("[data-wall-growth-key]:not(:disabled)")
        || (!nearest.hidden ? nearest : this.root.querySelector('[data-wall-action="cancel-growth"]'));
      fallback?.focus?.({ preventScroll: true });
      this.announce(available.length
        ? "That comb was just filled. Focus moved to another open direction."
        : "That comb was just filled. This patch is now surrounded; choose its nearest open edge or cancel.");
    }
  }

  beginGrowthIntent(cell, kind, opener = null, focusAnchor = null) {
    if (!cell?.id || !["reply", "branch"].includes(kind)) return false;
    if (this.bounds.readOnly) {
      this.toast("The Wall is open to read while Hive Watch is verified. New combs will open soon.");
      return false;
    }
    this.growthIntent = {
      kind,
      origin: cell,
      originalOrigin: cell,
      opener: opener || this.nodes.get(cellKey(cell.q, cell.r)) || this.stage,
      focusAnchor: focusAnchor || null,
      priorView: {
        zoom: this.zoom,
        cameraX: this.cameraX,
        cameraY: this.cameraY,
        viewportWidth: Number(window.visualViewport?.width) || window.innerWidth,
        viewportHeight: Number(window.visualViewport?.height) || window.innerHeight,
      },
    };
    // Deliberately does not scroll the page. A picker is always opened from a
    // comb the viewer just acted on, so the stage is already on screen, and a
    // scroll here would land its event after the fit and trigger a second one.
    this.renderGrowthSlots();
    this.fitGrowthIntent();
    const first = this.growthSlots.querySelector("[data-wall-growth-key]:not(:disabled)");
    if (first) {
      const [q, r] = first.dataset.wallGrowthKey.split(":").map(Number);
      this.selected = { q, r };
      this.updateTarget();
      first.focus({ preventScroll: true });
    } else {
      this.root.querySelector('[data-wall-action="nearest-growth"]')?.focus?.({ preventScroll: true });
    }
    this.announce(this.root.querySelector("[data-wall-growth-copy]")?.textContent || "Choose the next comb.");
    return true;
  }

  cancelGrowthIntent(options = {}) {
    const state = this.growthIntent;
    if (!state) return;
    this.growthIntent = null;
    this.renderGrowthSlots();
    this.restoreGrowthView(state, null, { restoreScroll: options.restoreView !== false });
    if (options.announce !== false) this.announce("Branch selection canceled.");
    if (options.restoreFocus !== false) this.restoreWallFocus(state.opener, state.focusAnchor);
  }

  chooseGrowthCoordinate(coordinate, opener = null) {
    const state = this.growthIntent;
    if (!state || !this.isGrowthCoordinate(coordinate)) {
      this.announce("That place is no longer open. Choose another glowing comb.");
      this.renderGrowthSlots();
      return false;
    }
    const origin = this.cellsById.get(state.origin.id) || state.origin;
    const options = state.kind === "branch" ? {
      intent: "branch",
      anchorCellIds: [origin.id],
      mode: "branch",
      choiceExplicit: true,
    } : {
      intent: "join",
      conversationId: origin.conversationId,
      replyToId: state.kind === "reply" ? origin.id : null,
      anchorCellIds: [origin.id],
      mode: state.kind === "reply" ? "reply" : "join",
      choiceExplicit: true,
    };
    const restoreOpener = state.opener || opener || this.stage;
    this.growthIntent = null;
    this.renderGrowthSlots();
    this.restoreGrowthView(state, coordinate);
    this.selected = { q: coordinate.q, r: coordinate.r };
    this.updateTarget();
    void this.preparePlacement(coordinate, { ...options, opener: restoreOpener });
    return true;
  }

  findConversationFrontierChoice(conversationId, near = { q: 0, r: 0 }) {
    if (!conversationId) return null;
    const cells = [...this.cells.values()]
      .filter((cell) => cell.conversationId === conversationId && cell.status === "visible")
      .sort((a, b) => hexDistance(a.q, a.r, near.q, near.r) - hexDistance(b.q, b.r, near.q, near.r));
    for (const anchor of cells) {
      const coordinate = neighbors(anchor.q, anchor.r)
        .find((candidate) => validCoordinate(candidate.q, candidate.r, this.bounds.radius)
          && !this.cells.has(cellKey(candidate.q, candidate.r)));
      if (coordinate) return { anchor, coordinate };
    }
    return null;
  }

  moveGrowthToNearestFrontier() {
    const state = this.growthIntent;
    if (!state) return;
    const found = this.findConversationFrontierChoice(state.origin.conversationId, state.origin);
    if (!found) {
      this.toast("Load more of this conversation to find an open edge.");
      return;
    }
    state.origin = found.anchor;
    if (state.kind === "reply") state.kind = "continue";
    this.renderGrowthSlots();
    this.fitGrowthIntent();
    const target = this.growthSlots.querySelector(`[data-wall-growth-key="${cellKey(found.coordinate.q, found.coordinate.r)}"]`)
      || this.growthSlots.querySelector("[data-wall-growth-key]:not(:disabled)");
    target?.focus?.({ preventScroll: true });
    this.announce(`The nearest open edge is beside ${found.anchor.author || "this conversation"}. Choose its glowing comb.`);
  }

  fitGrowthIntent() {
    const state = this.growthIntent;
    if (!state || !this.stage || !this.plane || !this.growthTray) return;
    // Disabled slots still communicate the occupied shape around the origin;
    // fit the whole local neighborhood, not only the remaining open exit.
    const buttons = [...this.growthSlots.querySelectorAll("[data-wall-growth-key]")];
    if (!buttons.length) return;
    // Deliberately never scrolls the page: this runs from the scroll listener,
    // and scrolling from inside it would refit, scroll, and refit again.
    // beginGrowthIntent puts the stage on screen once, before the first fit.

    // Everything below is measured in stage coordinates: the stage is the
    // visible viewport now, so the growth cluster only has to clear the
    // toolbar above it and the action tray below it. Re-measured on every
    // attempt because the tray can gain a line as its copy changes.
    const size = this.stageSize();
    const margin = 10;
    const usable = (box) => box.right - box.left > 24 && box.bottom - box.top > 24;
    const measure = () => {
      const stageRect = this.stage.getBoundingClientRect();
      const tray = this.growthTray.getBoundingClientRect();
      const toolbar = this.root.querySelector(".honey-toolbar")?.getBoundingClientRect();
      // Browser page-scale zoom shows only part of the layout viewport, so the
      // usable area is the stage intersected with the visual viewport. Fitting
      // to the stage alone would place slots a pinched-in viewer cannot see.
      const viewport = globalThis.window?.visualViewport;
      const viewLeft = Number(viewport?.offsetLeft) || 0;
      const viewTop = Number(viewport?.offsetTop) || 0;
      const viewRight = viewLeft + (Number(viewport?.width) || window.innerWidth);
      const viewBottom = viewTop + (Number(viewport?.height) || window.innerHeight);
      const toolbarOccludes = toolbar && toolbar.bottom > viewTop && toolbar.top < viewBottom;
      const trayOccludes = tray.height > 0 && tray.top < viewBottom;
      const box = {
        left: Math.max(margin, viewLeft - stageRect.left + margin),
        right: Math.min(size.width - margin, viewRight - stageRect.left - margin),
        top: Math.max(
          margin,
          viewTop - stageRect.top + margin,
          toolbarOccludes ? toolbar.bottom - stageRect.top + margin : 0,
        ),
        bottom: Math.min(
          size.height - margin,
          viewBottom - stageRect.top - margin,
          trayOccludes ? tray.top - stageRect.top - margin : Infinity,
        ),
      };
      // Mid-gesture the visual viewport can report a band that barely overlaps
      // the stage. Falling back to the stage keeps the picker fitted; bailing
      // out would leave it at the pre-gesture zoom, overflowing the screen.
      return { stageRect, box: usable(box) ? box : { left: margin, right: size.width - margin, top: margin, bottom: size.height - margin } };
    };

    const { stageRect, box: available } = measure();
    if (!usable(available)) return;
    this.lastGrowthTrayHeight = this.growthTray.getBoundingClientRect().height;

    const originPoint = axialToPixel(state.origin.q, state.origin.r, this.hexRadius);
    let minX = Infinity;
    let maxX = -Infinity;
    let minY = Infinity;
    let maxY = -Infinity;
    for (const button of buttons) {
      const [q, r] = button.dataset.wallGrowthKey.split(":").map(Number);
      const point = axialToPixel(q, r, this.hexRadius);
      const halfWidth = button.offsetWidth / 2 + 7;
      const halfHeight = button.offsetHeight / 2 + 7;
      minX = Math.min(minX, point.x - originPoint.x - halfWidth);
      maxX = Math.max(maxX, point.x - originPoint.x + halfWidth);
      minY = Math.min(minY, point.y - originPoint.y - halfHeight);
      maxY = Math.max(maxY, point.y - originPoint.y + halfHeight);
    }
    const clusterWidth = Math.max(1, maxX - minX);
    const clusterHeight = Math.max(1, maxY - minY);
    const clusterCenterX = (minX + maxX) / 2;
    const clusterCenterY = (minY + maxY) / 2;
    const fitZoom = Math.min(
      state.priorView?.zoom || this.zoom,
      (available.right - available.left) / clusterWidth,
      (available.bottom - available.top) / clusterHeight,
    );
    // The full-size comb buttons keep more than a 44px touch target well below
    // the ordinary Wall zoom floor, so a temporary lower floor lets all six
    // directions fit above the action tray in a short landscape viewport.
    this.zoom = clamp(fitZoom, 0.3, WALL_LIMITS.maxZoom);
    // Deliberately not clamped to the ordinary camera limits: the growth
    // cluster must stay reachable even when it sits on the wall's edge.
    this.cameraX = (available.left + available.right) / 2 - size.width / 2
      - (originPoint.x + clusterCenterX) * this.zoom;
    this.cameraY = (available.top + available.bottom) / 2
      - (this.world.originY + originPoint.y + clusterCenterY) * this.zoom;
    this.updatePlaneTransform();
    this.updateTarget();
  }

  // The tray gains and loses a line of wrapped copy as its message changes,
  // which moves the ceiling the cluster has to clear. Watching it is the only
  // way to refit after a change the fit itself could not have predicted.
  scheduleGrowthFit() {
    if (!this.growthIntent) return;
    if (this.growthFitFrame) cancelAnimationFrame(this.growthFitFrame);
    this.growthFitFrame = requestAnimationFrame(() => {
      this.growthFitFrame = null;
      this.fitGrowthIntent();
    });
  }

  restoreGrowthView(state, coordinate = null, options = {}) {
    const view = state?.priorView;
    if (!view) return;
    if (this.growthFitFrame) cancelAnimationFrame(this.growthFitFrame);
    this.growthFitFrame = null;
    this.zoom = this.clampZoom(view.zoom);
    this.cameraX = view.cameraX;
    this.cameraY = view.cameraY;
    this.clampCamera();
    this.updatePlaneTransform();
    const currentViewportWidth = Number(window.visualViewport?.width) || window.innerWidth;
    const currentViewportHeight = Number(window.visualViewport?.height) || window.innerHeight;
    const viewportChanged = Math.abs(currentViewportWidth - view.viewportWidth) > 1
      || Math.abs(currentViewportHeight - view.viewportHeight) > 1;
    if (coordinate) {
      this.revealCoordinate(coordinate.q, coordinate.r, "auto");
    } else if (options.restoreScroll !== false && viewportChanged && state.originalOrigin) {
      this.revealCoordinate(state.originalOrigin.q, state.originalOrigin.r, "auto");
    } else {
      this.updateTarget();
      this.scheduleViewport();
    }
  }

  async preparePlacement(coordinate, options = {}) {
    let intentGeneration = ++this.placementIntentGeneration;
    const activationGeneration = this.activationGeneration;
    let identityGeneration = this.identityGeneration;
    const q = Number(coordinate.q);
    const r = Number(coordinate.r);
    if (!validCoordinate(q, r)) {
      this.toast("That spot is beyond the Hive's growing edge.");
      return;
    }
    if (this.bounds.readOnly) {
      this.toast("The Wall is open to read while Hive Watch is verified. New combs will open soon.");
      return;
    }
    const existingDraft = normalizedPlacementDraft(this.draft, true);
    if (existingDraft
      && (existingDraft.q !== q || existingDraft.r !== r)
      && String(existingDraft.body || "").trim()
      && options.preserveExistingDraft !== false) {
      const move = window.confirm(`You already have an unfinished draft at comb ${existingDraft.q}, ${existingDraft.r}. Move that draft to comb ${q}, ${r}? Choose Cancel to keep its original address.`);
      if (!move) return;
      options = { ...options, body: existingDraft.body, requestId: null, preserveExistingDraft: false };
    }
    const context = await this.hydratePlacement(q, r);
    if (intentGeneration !== this.placementIntentGeneration
      || !this.isContextCurrent(activationGeneration, identityGeneration)) return;
    if (!context.ok) {
      this.toast(context.code === "out_of_bounds"
        ? "That spot is beyond the Hive's growing edge."
        : "The Hive could not verify that comb yet. Your current wall stays visible; try the spot again when it reconnects.");
      return;
    }
    const occupied = context.occupied;
    if (occupied) {
      this.openReader(occupied);
      return;
    }
    if (!this.storageScope) {
      const expectedScopeRequest = this.scopeRequestGeneration + 1;
      const resolvedScope = await this.syncStorageScope(true);
      if (this.scopeRequestGeneration !== expectedScopeRequest
        || !this.isActivationCurrent(activationGeneration)
        || !resolvedScope
        || resolvedScope !== this.storageScope) {
        this.toast("The Hive could not establish a safe identity for this comb. Nothing was posted; try again when the connection recovers.");
        return;
      }
      intentGeneration = ++this.placementIntentGeneration;
      identityGeneration = this.identityGeneration;
    }
    if (intentGeneration !== this.placementIntentGeneration
      || !this.isContextCurrent(activationGeneration, identityGeneration)) return;
    const groups = context.groups || adjacentConversationGroups(this.cells, q, r, this.conversations);
    const choices = context.choices;
    const requestedIntent = PLACEMENT_INTENTS.has(options.intent) ? options.intent : null;
    const isolatedIndependent = !requestedIntent && groups.length === 0;
    const desiredIntent = requestedIntent || (isolatedIndependent ? "independent" : null);
    const desiredConversation = options.conversationId || null;
    const choiceExplicit = options.choiceExplicit === true || isolatedIndependent;
    const choiceMade = Boolean(desiredIntent) && (choiceExplicit || isolatedIndependent);
    const state = {
      q,
      r,
      body: options.body || (this.draft && this.draft.q === q && this.draft.r === r ? this.draft.body || "" : ""),
      requestId: options.requestId || null,
      conversationId: desiredConversation,
      replyToId: options.replyToId || null,
      intent: desiredIntent,
      anchorCellIds: canonicalAnchorIds(options.anchorCellIds),
      choices,
      groups,
      mode: options.mode || (desiredConversation ? "join" : "root"),
      choiceMade,
      choiceExplicit,
      requiresConversationChoice: groups.length > 0 && !choiceMade,
      placementVerified: true,
    };
    if (!this.getUser()) {
      this.draft = state;
      if (this.saveDraft() === false) {
        this.toast("Create a profile first, then choose this comb again. This browser cannot safely carry the spot through identity setup.");
        return;
      }
      this.announce("Create a profile, then this exact comb and draft will reopen.");
      this.requestIdentity({ reason: "wall", coordinate: { q, r } });
      return;
    }
    this.openComposer(state, options.opener);
  }

  openComposer(state, opener = null) {
    this.placementIntentGeneration += 1;
    const generation = ++this.composeGeneration;
    this.composeBusy = false;
    this.composeBody.readOnly = false;
    const groups = Array.isArray(state.groups)
      ? state.groups
      : adjacentConversationGroups(this.cells, Number(state.q), Number(state.r), this.conversations);
    this.composeState = {
      generation,
      q: Number(state.q),
      r: Number(state.r),
      body: state.body || "",
      requestId: state.requestId || null,
      conversationId: state.conversationId || null,
      replyToId: state.replyToId || null,
      intent: PLACEMENT_INTENTS.has(state.intent) ? state.intent : null,
      anchorCellIds: canonicalAnchorIds(state.anchorCellIds),
      choices: Array.isArray(state.choices) ? state.choices : conversationChoices(this.cells, Number(state.q), Number(state.r), this.conversations),
      groups,
      mode: state.mode || (state.conversationId ? "join" : "root"),
      choiceMade: state.choiceMade !== false,
      choiceExplicit: state.choiceExplicit === true,
      requiresConversationChoice: state.requiresConversationChoice === true,
      choiceLocked: state.choiceLocked === true || (state.choiceExplicit === true && ["reply", "branch"].includes(state.mode)),
      placementVerified: state.placementVerified !== false,
    };
    const submit = this.root.querySelector("[data-wall-compose-submit]");
    submit.disabled = this.bounds.readOnly || !this.composeState.choiceMade || !this.composeState.intent;
    submit.textContent = this.placementSubmitLabel();
    this.root.querySelector("[data-wall-compose-cancel]").textContent = "Keep as draft";
    this.composeBody.value = this.composeState.body;
    this.root.querySelector("[data-wall-compose-count]").textContent = `${[...this.composeBody.value].length} / 600`;
    this.root.querySelector("[data-wall-compose-place]").textContent = "This comb keeps its place while every nearby conversation stays free to grow.";
    const choiceField = this.root.querySelector("[data-wall-conversation-choices]");
    choiceField.disabled = false;
    const choiceList = this.root.querySelector("[data-wall-conversation-options]");
    choiceList.replaceChildren();
    const relationChoices = this.placementChoicesForGroups(groups);
    if (relationChoices.length && !this.composeState.choiceLocked) {
      choiceField.hidden = false;
      setTextIfChanged(this.root.querySelector("[data-wall-conversation-legend]"), groups.length > 1
        ? "Conversations meet at this comb. What should happen here?"
        : "How should this comb connect?");
      for (const choice of relationChoices) choiceList.append(this.conversationRadio(choice));
    } else if (this.composeState.requiresConversationChoice) {
      choiceField.hidden = false;
      const pending = document.createElement("p");
      pending.className = "honey-choice-pending";
      pending.textContent = "Nearby conversations must reload before you choose where this comb belongs.";
      choiceList.append(pending);
    } else {
      choiceField.hidden = true;
    }
    this.paintComposerIntentCopy();
    this.hideComposeError();
    this.draft = { ...this.composeState };
    this.saveDraft();
    const firstChoice = !choiceField.hidden && !this.composeState.choiceMade
      ? choiceList.querySelector("input[type=radio]")
      : null;
    this.showDialog(this.compose, firstChoice || this.composeBody, opener);
  }

  placementChoicesForGroups(groups) {
    const choices = [];
    for (const group of groups) {
      const anchor = group.anchors[0]?.cell;
      if (!anchor) continue;
      choices.push({
        intent: "join",
        conversationId: group.conversationId,
        replyToId: null,
        anchorCellIds: [anchor.id],
        group,
      });
      choices.push({
        intent: "branch",
        conversationId: null,
        replyToId: null,
        anchorCellIds: [anchor.id],
        group,
      });
    }
    if (groups.length > 1) {
      choices.push({
        intent: "meet",
        conversationId: null,
        replyToId: null,
        anchorCellIds: groups.map((group) => group.anchors[0]?.cell?.id).filter(Boolean),
        groups,
      });
    }
    choices.push({ intent: "independent", conversationId: null, replyToId: null, anchorCellIds: [] });
    return choices;
  }

  placementChoiceSelected(choice) {
    return this.composeState.choiceMade
      && this.composeState.intent === choice.intent
      && (this.composeState.conversationId || null) === (choice.conversationId || null)
      && canonicalAnchorIds(this.composeState.anchorCellIds).join(",") === canonicalAnchorIds(choice.anchorCellIds).join(",");
  }

  conversationRadio(choice) {
    const label = document.createElement("label");
    label.className = "honey-conversation-option";
    const input = document.createElement("input");
    input.type = "radio";
    input.name = "honeyConversation";
    input.value = choice.intent;
    input.checked = this.placementChoiceSelected(choice);
    input.required = this.composeState.requiresConversationChoice && !this.composeState.choiceMade;
    input.addEventListener("change", () => {
      this.composeState.intent = choice.intent;
      this.composeState.conversationId = choice.conversationId || null;
      this.composeState.replyToId = choice.replyToId || null;
      this.composeState.anchorCellIds = canonicalAnchorIds(choice.anchorCellIds);
      this.composeState.mode = choice.intent;
      this.composeState.choiceMade = true;
      this.composeState.choiceExplicit = true;
      this.composeState.requiresConversationChoice = false;
      this.composeState.requestId = null;
      const submit = this.root.querySelector("[data-wall-compose-submit]");
      submit.disabled = this.bounds.readOnly || this.composeBusy;
      submit.textContent = this.placementSubmitLabel();
      this.paintComposerIntentCopy();
      this.saveDraft();
    });
    const copy = document.createElement("span");
    copy.className = "honey-conversation-copy";
    const title = document.createElement("strong");
    const detail = document.createElement("small");
    if (choice.intent === "independent") {
      title.textContent = "Start on its own";
      detail.textContent = "Nearby in space, but not linked to another conversation.";
    } else if (choice.intent === "branch") {
      const anchor = choice.group.anchors[0];
      title.textContent = "Start a new branch";
      detail.textContent = `Bud from ${anchor.cell.author || "this conversation"}'s ${anchor.directionLabel} comb; both paths can keep growing.`;
    } else if (choice.intent === "meet") {
      title.textContent = "Meet them here";
      detail.textContent = `Start a new conversation linked to ${choice.groups.length} nearby groups. Nothing is merged or moved.`;
    } else {
      const anchor = choice.group.anchors[0];
      const conversation = this.conversations.get(choice.conversationId);
      const names = conversation && Array.isArray(conversation.participants)
        ? conversation.participants.map((participant) => participant.name).filter(Boolean).slice(0, 3)
        : [];
      title.textContent = names.length ? `Continue with ${names.join(", ")}` : `Continue with ${anchor.cell.author || "the nearby conversation"}`;
      const excerpt = String(anchor.cell.body || "").replace(/\s+/g, " ").trim().slice(0, 72);
      detail.textContent = `${anchor.directionLabel}${choice.group.anchors.length > 1 ? ` · touches on ${choice.group.anchors.length} sides` : ""}${excerpt ? ` · “${excerpt}${String(anchor.cell.body || "").length > 72 ? "…" : ""}”` : ""}`;
    }
    copy.append(title, detail);
    label.append(input, copy);
    return label;
  }

  placementSubmitLabel() {
    if (this.composeState?.mode === "reply") return "Place this reply";
    return {
      join: "Continue conversation",
      branch: "Start this branch",
      meet: "Meet here",
      independent: "Start conversation",
    }[this.composeState?.intent] || "Choose a connection";
  }

  paintComposerIntentCopy() {
    const intent = this.composeState?.intent;
    const copy = {
      join: [this.composeState?.mode === "reply" ? "Replying beside a comb" : "Continuing nearby", this.composeState?.mode === "reply" ? "Reply from this comb" : "Continue this conversation"],
      branch: ["Budding a new path", "Start a new branch"],
      meet: ["Nearby groups crossing paths", "Let these conversations meet"],
      independent: ["New patch", "Start something here"],
    }[intent] || ["Open comb", "Choose how this comb connects"];
    setTextIfChanged(this.root.querySelector("[data-wall-compose-kicker]"), copy[0]);
    setTextIfChanged(this.root.querySelector("[data-wall-compose-title]"), copy[1]);
  }

  async submitCompose() {
    const state = this.composeState;
    if (!state) return;
    const generation = state.generation;
    const identityGeneration = this.identityGeneration;
    const body = this.composeBody.value.trim();
    if (!body || [...body].length > WALL_LIMITS.bodyCharacters) {
      this.showComposeError("Write between 1 and 600 characters. Nothing will be silently cut off.");
      return;
    }
    if (state.editing) {
      await this.commitEdit(body);
      return;
    }
    if (!this.getUser()) {
      state.body = body;
      this.draft = { ...state };
      if (this.saveDraft() === false) return;
      this.closeDialog(this.compose);
      this.requestIdentity({ reason: "wall", coordinate: { q: state.q, r: state.r } });
      return;
    }
    const submit = this.root.querySelector("[data-wall-compose-submit]");
    submit.disabled = true;
    submit.textContent = "Checking this comb…";
    this.composeBody.readOnly = true;
    this.composeBusy = true;
    const choiceField = this.root.querySelector("[data-wall-conversation-choices]");
    choiceField.disabled = true;
    this.hideComposeError();
    try {
      const replayFrozenCommand = Boolean(
        state.requestId && String(state.body || "").trim() === body
      );
      if (!replayFrozenCommand) {
        // Editing the command payload intentionally abandons any older
        // idempotency key; ordinary input/radio handlers do this first too.
        state.requestId = null;
        const context = await this.hydratePlacement(state.q, state.r);
        if (identityGeneration !== this.identityGeneration
          || this.composeState !== state || generation !== this.composeGeneration) return;
        if (!context.ok) {
          state.placementVerified = false;
          this.showComposeError("The Hive could not verify this comb and its neighbors. Your draft remains open here; reconnect before placing it.");
          return;
        }
        if (context.occupied) {
          this.draft = { ...state, body };
          this.saveDraft();
          const next = this.findOpenNeighbor({ q: state.q, r: state.r }, state.conversationId);
          if (next) {
            this.selected = next;
            this.updateTarget();
            this.showComposeError("Someone filled that comb first. Your whole draft remains here. Move it only if the highlighted open comb works for you.", next);
          } else {
            this.showComposeError("Someone filled that comb first. Your whole draft remains here, but no open neighboring comb is loaded yet.");
          }
          return;
        }
        state.placementVerified = true;
        state.choices = context.choices;
        state.groups = context.groups || adjacentConversationGroups(this.cells, state.q, state.r, this.conversations);
        if (!state.choiceMade || !PLACEMENT_INTENTS.has(state.intent)) {
          const opener = this.dialogOpeners.get(this.compose);
          this.openComposer({
            ...state,
            body,
            choices: context.choices,
            groups: state.groups,
            choiceMade: false,
            requiresConversationChoice: true,
            placementVerified: true,
          }, opener);
          this.showComposeError("Choose how this comb connects before placing it.");
          this.root.querySelector("[data-wall-conversation-options] input[type=radio]")?.focus?.({ preventScroll: true });
          return;
        }
        const adjacentIds = new Set(state.groups.flatMap((group) => group.anchors.map((anchor) => anchor.cell.id)));
        const missingAnchor = canonicalAnchorIds(state.anchorCellIds).some((cellId) => !adjacentIds.has(cellId));
        if (state.intent !== "independent" && missingAnchor) {
          const opener = this.dialogOpeners.get(this.compose);
          this.openComposer({
            ...state,
            body,
            choices: context.choices,
            groups: state.groups,
            intent: null,
            conversationId: null,
            replyToId: null,
            anchorCellIds: [],
            choiceMade: false,
            choiceExplicit: false,
            choiceLocked: false,
            requiresConversationChoice: true,
            placementVerified: true,
          }, opener);
          this.showComposeError("A linked comb changed while you were writing. Your message is untouched; choose the current connection again.");
          this.root.querySelector("[data-wall-conversation-options] input[type=radio]")?.focus?.({ preventScroll: true });
          return;
        }
      }
      const requestId = state.requestId || randomRequestId();
      state.requestId = requestId;
      state.body = body;
      this.draft = { ...state };
      this.saveDraft();
      submit.textContent = "Placing…";
      const result = await this.backend.wallPlaceCell({
        requestId,
        q: state.q,
        r: state.r,
        body,
        intent: state.intent,
        conversationId: state.conversationId,
        replyToId: state.replyToId,
        anchorCellIds: canonicalAnchorIds(state.anchorCellIds),
      });
      if (identityGeneration !== this.identityGeneration) return;
      if (this.composeState !== state || generation !== this.composeGeneration) {
        if (result?.ok && result.cell) {
          if (this.draft?.requestId === state.requestId) this.clearDraft();
          this.markSeen(result.cell.id);
          const rebuild = this.captureProjectionRebuild();
          await this.rebuildAfterConfirmedWrite(rebuild);
          this.toast("Your earlier comb was placed while you moved on.");
          if (this.onPlaced) this.onPlaced(result.cell);
        }
        return;
      }
      if (result && result.ok && result.cell) {
        // A confirmed response may jump across unseen global revisions. Never
        // splice just this cell into an older projection: clear first, then
        // authoritatively rebuild the current wall/list window.
        const rebuild = this.captureProjectionRebuild();
        this.composeBusy = false;
        this.clearDraft();
        this.closeDialog(this.compose);
        this.markSeen(result.cell.id);
        await this.rebuildAfterConfirmedWrite(rebuild);
        if (this.active && this.mode === "wall") await this.focusCell(result.cell.id);
        const placedCopy = {
          join: state.mode === "reply" ? "Your reply grew this conversation." : "Your comb continued this conversation.",
          branch: "Your new branch can now grow in any direction.",
          meet: "These nearby conversations now meet at a new patch.",
          independent: "Your new conversation is growing in the Hive.",
        }[state.intent] || "Your comb is part of the shared Hive.";
        this.toast(placedCopy);
        if (this.onPlaced) this.onPlaced(result.cell);
      } else if (result && result.code === "occupied") {
        state.requestId = null;
        this.draft = { ...state };
        this.saveDraft();
        const next = this.findOpenNeighbor({ q: state.q, r: state.r }, state.conversationId);
        if (next) {
          this.selected = next;
          this.updateTarget();
          this.showComposeError("Someone filled that comb first. Your whole draft remains here. Move it only if the highlighted open comb works for you.", next);
        } else {
          this.showComposeError("Someone filled that comb first. Your whole draft remains here, but no open neighboring comb is loaded yet.");
        }
      } else if (result && (result.code === "not_adjacent" || result.code === "invalid_reply")) {
        state.requestId = null;
        this.draft = { ...state };
        this.saveDraft();
        const next = this.findConversationFrontier(state.conversationId, state);
        if (next) {
          this.selected = next;
          this.updateTarget();
          this.showComposeError("That conversation grew away from this spot. Your draft remains here. Move it only if the highlighted edge comb works for you.", next);
        } else {
          this.showComposeError("That conversation grew away from this spot. Your draft remains here, but its open edge is not loaded yet.");
        }
      } else if (result && result.code === "stale_anchors") {
        state.requestId = null;
        this.draft = { ...state, body };
        this.saveDraft();
        const context = await this.hydratePlacement(state.q, state.r);
        if (identityGeneration !== this.identityGeneration
          || this.composeState !== state || generation !== this.composeGeneration) return;
        if (context.ok && !context.occupied) {
          const opener = this.dialogOpeners.get(this.compose);
          this.openComposer({
            ...state,
            body,
            requestId: null,
            choices: context.choices,
            groups: context.groups || [],
            intent: null,
            conversationId: null,
            replyToId: null,
            anchorCellIds: [],
            choiceMade: false,
            choiceExplicit: false,
            choiceLocked: false,
            requiresConversationChoice: true,
            placementVerified: true,
          }, opener);
          this.showComposeError("A nearby conversation changed while this comb was being placed. Your message is untouched; choose the current connection again.");
          this.root.querySelector("[data-wall-conversation-options] input[type=radio]")?.focus?.({ preventScroll: true });
        } else {
          this.showComposeError(`${result.message || "A nearby conversation changed."} Your draft is still here; reconnect and choose again.`);
        }
      } else if (result && result.ok === false) {
        state.requestId = null;
        this.draft = { ...state };
        this.saveDraft();
        if (result.code === "read_only" || result.code === "out_of_bounds") void this.refreshBounds();
        this.showComposeError(`${result.message || "The wall could not place that comb."} Your draft is still here.`);
      } else {
        this.showComposeError("The wall did not confirm that placement. Your draft is still here.");
      }
    } catch (error) {
      if (this.composeState === state && generation === this.composeGeneration) {
        this.showComposeError(`${error.message} Your draft is still open here; retrying will use the same idempotent request.`);
      }
      this.dataStale = true;
      this.setConnectionState(navigator.onLine ? "stale" : "offline");
    } finally {
      if (this.composeState === state && generation === this.composeGeneration) {
        this.composeBusy = false;
        submit.disabled = this.bounds.readOnly;
        submit.textContent = typeof this.placementSubmitLabel === "function"
          ? this.placementSubmitLabel()
          : "Place this comb";
        this.composeBody.readOnly = false;
        choiceField.disabled = false;
      }
    }
  }

  acceptComposeSuggestion() {
    const state = this.composeState;
    if (!state) return;
    if (state.editing && state.editConflictCell) {
      const fresh = state.editConflictCell;
      const opener = this.dialogOpeners.get(this.compose);
      this.saveEditDraft();
      this.closeDialog(this.compose, { restoreFocus: false });
      this.openReader(fresh, opener);
      return;
    }
    const coordinate = state.suggestedCoordinate;
    if (!coordinate || this.cells.has(cellKey(coordinate.q, coordinate.r))) {
      this.showComposeError("That suggested comb is no longer open. Your draft has not moved.");
      return;
    }
    const choices = conversationChoices(this.cells, coordinate.q, coordinate.r, this.conversations);
    const groups = adjacentConversationGroups(this.cells, coordinate.q, coordinate.r, this.conversations);
    const adjacentIds = new Set(groups.flatMap((group) => group.anchors.map((anchor) => anchor.cell.id)));
    const anchorsRemainAdjacent = state.intent === "independent"
      || canonicalAnchorIds(state.anchorCellIds).every((cellId) => adjacentIds.has(cellId));
    const body = this.composeBody.value;
    this.selected = { q: coordinate.q, r: coordinate.r };
    this.updateTarget();
    this.openComposer({
      ...state,
      q: coordinate.q,
      r: coordinate.r,
      body,
      requestId: null,
      choices,
      groups,
      intent: anchorsRemainAdjacent ? state.intent : null,
      conversationId: anchorsRemainAdjacent ? state.conversationId : null,
      replyToId: anchorsRemainAdjacent ? state.replyToId : null,
      anchorCellIds: anchorsRemainAdjacent ? state.anchorCellIds : [],
      mode: anchorsRemainAdjacent ? state.mode : "root",
      choiceMade: anchorsRemainAdjacent && state.choiceMade,
      choiceExplicit: anchorsRemainAdjacent && state.choiceExplicit,
      choiceLocked: anchorsRemainAdjacent && state.choiceLocked,
      requiresConversationChoice: !anchorsRemainAdjacent || (groups.length > 0 && !state.choiceMade),
      placementVerified: true,
      suggestedCoordinate: null,
    });
    this.announce(anchorsRemainAdjacent
      ? "Draft moved to the highlighted open comb with its chosen connection intact."
      : "Draft moved, but its old connection no longer reaches this comb. Choose the current relationship before placing it.");
  }

  findOpenNeighbor(origin, conversationId = null) {
    const direct = neighbors(origin.q, origin.r);
    const candidates = conversationId
      ? direct.filter((coordinate) => {
        return neighbors(coordinate.q, coordinate.r).some((neighbor) => this.cells.get(cellKey(neighbor.q, neighbor.r))?.conversationId === conversationId);
      })
      : direct;
    return candidates.find((coordinate) => validCoordinate(coordinate.q, coordinate.r, this.bounds.radius) && !this.cells.has(cellKey(coordinate.q, coordinate.r))) || null;
  }

  findNearestOpen(origin = { q: 0, r: 0 }) {
    const start = validCoordinate(origin.q, origin.r, this.bounds.radius) ? origin : { q: 0, r: 0 };
    const queue = [{ q: start.q, r: start.r }];
    const seen = new Set([cellKey(start.q, start.r)]);
    while (queue.length) {
      const current = queue.shift();
      if (!this.cells.has(cellKey(current.q, current.r))) return current;
      for (const candidate of neighbors(current.q, current.r)) {
        const key = cellKey(candidate.q, candidate.r);
        if (seen.has(key) || !validCoordinate(candidate.q, candidate.r, this.bounds.radius)) continue;
        seen.add(key);
        queue.push(candidate);
      }
    }
    return null;
  }

  findConversationFrontier(conversationId, near = { q: 0, r: 0 }) {
    if (!conversationId) return this.findOpenNeighbor(near);
    const cells = [...this.cells.values()]
      .filter((cell) => cell.conversationId === conversationId)
      .sort((a, b) => hexDistance(a.q, a.r, near.q, near.r) - hexDistance(b.q, b.r, near.q, near.r));
    for (const cell of cells) {
      const open = this.findOpenNeighbor(cell, conversationId);
      if (open) return open;
    }
    return null;
  }

  openReader(cell, opener = null) {
    cell = this.cellsById.get(cell?.id) || this.mergeCell(cell) || cell;
    if (!cell?.id) return;
    this.placementIntentGeneration += 1;
    this.readerGeneration += 1;
    this.setReaderBusy(false);
    if (this.readerCell?.id !== cell.id || this.readerCell?.contentVersion !== cell.contentVersion) {
      this.reportRequest = null;
      this.operatorCaseRequest = null;
    }
    this.readerCell = cell;
    this.paintReaderCell(cell);
    this.markSeen(cell.id);
    this.showDialog(this.reader, this.reader.querySelector("[data-wall-reader-action]:not([hidden])"), opener);
  }

  paintReaderCell(cell, options = {}) {
    if (!cell) return;
    const conversation = this.conversations.get(cell.conversationId);
    setTextIfChanged(this.root.querySelector("[data-wall-reader-author]"), cell.author || (cell.status === "visible" ? "Hive member" : "Open space kept"));
    const relationKind = cell.replyToId ? "Reply comb"
      : cell.origin?.kind === "branch" ? "Branch root"
        : cell.origin?.kind === "meet" ? "Meeting comb"
          : conversation?.rootCellId === cell.id ? "Conversation root" : "Conversation continuation";
    setTextIfChanged(this.root.querySelector("[data-wall-reader-kicker]"), relationKind);
    setTextIfChanged(this.root.querySelector("[data-wall-reader-meta]"), `${cell.membership === "alumni" ? "DHG alumni" : cell.membership === "guest" ? "Guest" : "Hive"} · ${timeLabel(cell.createdAt)} · comb ${cell.q}, ${cell.r}`);
    setTextIfChanged(this.root.querySelector("[data-wall-reader-body]"), statusText(cell));
    const participants = this.root.querySelector("[data-wall-reader-participants]");
    const participantNames = conversation && Array.isArray(conversation.participants)
      ? conversation.participants.map((participant) => participant.name).filter(Boolean).join(", ")
      : "";
    if (participants.dataset.wallParticipantNames !== participantNames) {
      participants.dataset.wallParticipantNames = participantNames;
      participants.replaceChildren();
      if (participantNames) {
        const label = document.createElement("span");
        label.textContent = "In this conversation: ";
        const names = document.createElement("strong");
        names.textContent = participantNames;
        participants.append(label, names);
      }
    }
    const relations = this.root.querySelector("[data-wall-reader-relations]");
    const relationFingerprint = JSON.stringify(cell.origin || null);
    if (relations.dataset.wallRelations !== relationFingerprint) {
      relations.dataset.wallRelations = relationFingerprint;
      relations.replaceChildren();
      if (cell.origin?.anchors?.length) {
        const heading = document.createElement("strong");
        heading.textContent = cell.origin.kind === "meet" ? "Connected nearby:" : "Branched from:";
        relations.append(heading);
        for (const link of cell.origin.anchors) {
          const anchor = this.cellsById.get(link.cellId);
          if (anchor) {
            const button = document.createElement("button");
            button.type = "button";
            button.className = "honey-relation-link";
            button.dataset.cellKey = cellKey(anchor.q, anchor.r);
            button.textContent = `${anchor.author || "Nearby comb"} · ${HEX_DIRECTION_LABELS[Number(link.direction)] || "beside this patch"}`;
            relations.append(button);
          } else {
            const text = document.createElement("span");
            text.textContent = "A nearby conversation";
            relations.append(text);
          }
        }
      }
    }
    const canInteract = cell.status === "visible";
    const canWrite = canInteract && !this.bounds.readOnly;
    const reply = this.root.querySelector('[data-wall-reader-action="reply"]');
    const branch = this.root.querySelector('[data-wall-reader-action="branch"]');
    const edit = this.root.querySelector('[data-wall-reader-action="edit"]');
    const remove = this.root.querySelector('[data-wall-reader-action="remove"]');
    const report = this.root.querySelector('[data-wall-reader-action="report"]');
    const watchOpen = this.root.querySelector('[data-wall-reader-action="watch-open"]');
    reply.hidden = !canWrite;
    branch.hidden = !canWrite;
    edit.hidden = !canWrite || !cell.isMine;
    remove.hidden = this.bounds.readOnly || !cell.isMine || cell.status === "author_removed";
    report.hidden = !canWrite || cell.isMine || cell.viewerReported;
    watchOpen.hidden = !canInteract || !this.operator;
    const reportForm = this.root.querySelector("[data-wall-report-form]");
    const operatorForm = this.root.querySelector("[data-wall-operator-form]");
    const reportFormAllowed = canWrite && !cell.isMine && !cell.viewerReported;
    const operatorFormAllowed = canInteract && Boolean(this.operator);
    const activeBefore = globalThis.document?.activeElement;
    if (!options.preserveForms || !reportFormAllowed) reportForm.hidden = true;
    if (!options.preserveForms || !operatorFormAllowed) operatorForm.hidden = true;
    if (!options.preserveForms) {
      this.root.querySelector("[data-wall-report-reason]").value = "";
      this.root.querySelector("[data-wall-operator-reason]").value = "";
    }
    if (activeBefore && this.reader?.contains(activeBefore)
      && (activeBefore.hidden || activeBefore.closest?.("[hidden]"))) {
      this.focusReaderFallback(report);
    }
    this.setReaderLocked(this.readerBusy);
  }

  focusReaderFallback(preferred = null) {
    const available = (control) => Boolean(control
      && !control.disabled
      && !control.hidden
      && !control.closest?.("[hidden]"));
    const fallback = available(preferred) ? preferred
      : [...(this.reader?.querySelectorAll?.("[data-wall-reader-action], [data-wall-close=\"reader\"]") || [])]
        .find(available) || this.reader;
    fallback?.focus?.({ preventScroll: true });
    return fallback || null;
  }

  setReaderLocked(locked) {
    if (!this.reader) return;
    for (const control of this.reader.querySelectorAll("button, select, textarea")) {
      control.disabled = Boolean(locked) && !control.matches('[data-wall-close="reader"]');
    }
  }

  setReaderBusy(locked) {
    this.readerBusy = Boolean(locked);
    this.setReaderLocked(this.readerBusy);
  }

  async runReaderAction(action) {
    if (this.readerBusy) return;
    const cell = this.readerCell;
    if (!cell) return;
    if (this.bounds.readOnly && ["reply", "branch", "edit", "remove", "report"].includes(action)) {
      this.toast("The Wall is open to read while Hive Watch is verified. New combs will open soon.");
      return;
    }
    const opener = this.dialogOpeners.get(this.reader);
    const openerAnchor = this.dialogFocusAnchors.get(this.reader);
    if (action === "reply" || action === "branch") {
      this.closeDialog(this.reader, { restoreFocus: false });
      if (this.mode === "list") {
        this.setMode("wall");
        this.stage?.focus?.({ preventScroll: true });
        this.announce("Opening the spatial Wall so you can choose one of six directions.");
        const refreshed = await this.refreshVisible(true);
        const current = this.cellsById.get(cell.id);
        if (!refreshed || !current || current.status !== "visible") {
          this.toast("That comb changed while the spatial Wall opened. Nothing was posted; choose its current comb again.");
          this.stage?.focus?.({ preventScroll: true });
          return;
        }
        this.revealCoordinate(current.q, current.r, "auto");
        this.beginGrowthIntent(current, action === "branch" ? "branch" : "reply", this.stage, null);
      } else {
        this.beginGrowthIntent(cell, action === "branch" ? "branch" : "reply", opener, openerAnchor);
      }
    } else if (action === "show") {
      this.closeDialog(this.reader, { restoreFocus: false });
      this.setMode("wall");
      this.revealCoordinate(cell.q, cell.r);
      this.restoreWallFocus(this.stage);
    } else if (action === "edit") {
      this.closeDialog(this.reader, { restoreFocus: false });
      this.openEditComposer(cell, opener);
    } else if (action === "remove") {
      await this.removeCell(cell);
    } else if (action === "report") {
      this.root.querySelector("[data-wall-report-form]").hidden = false;
      this.root.querySelector("[data-wall-report-reason]").focus();
    } else if (action === "cancel-report") {
      this.root.querySelector("[data-wall-report-form]").hidden = true;
      this.focusReaderFallback(this.root.querySelector('[data-wall-reader-action="report"]'));
    } else if (action === "watch-open") {
      this.root.querySelector("[data-wall-operator-form]").hidden = false;
      this.root.querySelector("[data-wall-operator-reason]").focus();
    } else if (action === "cancel-watch-open") {
      this.root.querySelector("[data-wall-operator-form]").hidden = true;
      this.focusReaderFallback(this.root.querySelector('[data-wall-reader-action="watch-open"]'));
    }
  }

  openEditComposer(cell, opener = null) {
    if (this.editDraft
      && this.editDraft.cellId !== cell.id
      && String(this.editDraft.body || "").trim()
      && !window.confirm("You already have an unfinished edit for another comb. Replace that edit draft with this one?")) {
      this.restoreWallFocus(opener);
      return false;
    }
    this.placementIntentGeneration += 1;
    const generation = ++this.composeGeneration;
    this.composeBusy = false;
    this.composeBody.readOnly = false;
    this.root.querySelector("[data-wall-conversation-choices]").disabled = false;
    this.root.querySelector("[data-wall-compose-submit]").disabled = false;
    const saved = this.editDraft && this.editDraft.cellId === cell.id ? this.editDraft : null;
    const body = saved ? saved.body || "" : cell.body || "";
    this.composeState = {
      editing: true,
      generation,
      cellId: cell.id,
      expectedVersion: cell.version,
      q: cell.q,
      r: cell.r,
      body,
      requestId: saved && saved.expectedVersion === cell.version ? saved.requestId || null : null,
      recoveredFromVersion: saved && saved.expectedVersion !== cell.version ? saved.expectedVersion : null,
    };
    this.composeBody.value = body;
    this.root.querySelector("[data-wall-compose-kicker]").textContent = "Editing your comb";
    this.root.querySelector("[data-wall-compose-title]").textContent = "Update this message";
    this.root.querySelector("[data-wall-compose-place]").textContent = `Comb ${cell.q}, ${cell.r} stays in place.`;
    this.root.querySelector("[data-wall-conversation-choices]").hidden = true;
    this.root.querySelector("[data-wall-compose-submit]").textContent = "Save this comb";
    this.root.querySelector("[data-wall-compose-cancel]").textContent = "Keep edit draft";
    this.root.querySelector("[data-wall-compose-count]").textContent = `${[...this.composeBody.value].length} / 600`;
    this.hideComposeError();
    this.saveEditDraft();
    this.showDialog(this.compose, this.composeBody, opener);
    if (this.composeState.recoveredFromVersion) {
      this.showComposeError("This saved edit began from an older version. You reviewed the latest comb; check your draft carefully before saving over it.");
    }
    return true;
  }

  async submitEdit() {
    // Retained as a named operation for integration tests and future toolbar use.
    const state = this.composeState;
    if (!state || !state.editing) return null;
    const body = this.composeBody.value.trim();
    const requestId = state.requestId || randomRequestId();
    state.requestId = requestId;
    this.saveEditDraft();
    return this.backend.wallEditCell({
      requestId,
      cellId: state.cellId,
      expectedVersion: state.expectedVersion,
      body,
    });
  }

  async commitEdit(body) {
    const state = this.composeState;
    if (!state || !state.editing) return;
    const generation = state.generation;
    const identityGeneration = this.identityGeneration;
    const submit = this.root.querySelector("[data-wall-compose-submit]");
    submit.disabled = true;
    submit.textContent = "Saving…";
    this.composeBody.readOnly = true;
    this.composeBusy = true;
    try {
      const result = await this.submitEdit();
      if (identityGeneration !== this.identityGeneration) return;
      if (this.composeState !== state || generation !== this.composeGeneration) {
        if (result?.ok && result.cell) {
          const rebuild = this.captureProjectionRebuild();
          await this.rebuildAfterConfirmedWrite(rebuild);
          this.toast("Your earlier edit finished while you moved on.");
        }
        return;
      }
      if (result && result.ok && result.cell) {
        const rebuild = this.captureProjectionRebuild();
        this.clearEditDraft();
        this.composeBusy = false;
        this.composeState = null;
        this.closeDialog(this.compose);
        await this.rebuildAfterConfirmedWrite(rebuild);
        this.toast("Your comb was updated in place. The Wall reloaded its authoritative position.");
      } else if (result && result.code === "version_conflict") {
        state.requestId = null;
        this.saveEditDraft();
        let fresh = null;
        try {
          const exactCacheGeneration = this.publicCacheGeneration;
          const contextCurrent = () => (
            identityGeneration === this.identityGeneration
            && this.composeState === state
            && generation === this.composeGeneration
            && exactCacheGeneration === this.publicCacheGeneration
          );
          const latest = await this.readAuthoritativeCell(state.cellId, contextCurrent);
          if (latest?.rebuildRequired) {
            await this.rebuildPublicProjection(latest.rebuild);
          } else if (latest?.cell && contextCurrent()) {
            this.mergeCell(latest.cell);
            fresh = latest.cell;
          }
        } catch {
          // The edit remains locally recoverable even if the fresh read fails.
        }
        if (fresh) {
          state.editConflictCell = fresh;
          this.showComposeError("This comb changed on another screen. Your edit draft remains here; review the latest version before deciding what to save.", { editConflict: true });
        } else {
          this.showComposeError("This comb changed on another screen. Your edit draft remains here; reopen it before saving so newer words are not overwritten.");
        }
      } else if (result && result.ok === false) {
        state.requestId = null;
        this.saveEditDraft();
        this.showComposeError(`${result.message || "The wall could not save that edit."} Your edit draft is still here.`);
      } else {
        this.showComposeError("The wall did not confirm that edit. Your edit draft is still here.");
      }
    } catch (error) {
      if (this.composeState === state && generation === this.composeGeneration) this.showComposeError(`${error.message} Your edit is still here.`);
    } finally {
      if (this.composeState === state && generation === this.composeGeneration) {
        this.composeBusy = false;
        submit.disabled = false;
        submit.textContent = "Save this comb";
        this.composeBody.readOnly = false;
      }
    }
  }

  async removeCell(cell) {
    if (this.readerBusy) return;
    if (!window.confirm("Remove this message? Its comb will remain as a tombstone so the wall never collapses.")) return;
    const readerGeneration = this.readerGeneration;
    const identityGeneration = this.identityGeneration;
    this.setReaderBusy(true);
    try {
      const result = await this.backend.wallRemoveCell({
        requestId: randomRequestId(),
        cellId: cell.id,
        expectedVersion: cell.version,
      });
      if (identityGeneration !== this.identityGeneration
        || this.readerCell?.id !== cell.id || readerGeneration !== this.readerGeneration) return;
      if (result?.ok && result.cell) {
        const rebuild = this.captureProjectionRebuild();
        this.setReaderBusy(false);
        this.closeDialog(this.reader);
        this.readerCell = null;
        this.reportRequest = null;
        await this.rebuildAfterConfirmedWrite(rebuild);
        this.toast("Your message was removed; its place in the Hive remains. The Wall reloaded authoritatively.");
      } else if (result?.ok && result.code === "already_removed") {
        const rebuild = this.captureProjectionRebuild();
        this.setReaderBusy(false);
        this.closeDialog(this.reader);
        this.readerCell = null;
        this.reportRequest = null;
        await this.rebuildAfterConfirmedWrite(rebuild);
        this.toast("That message was already removed; its comb remains in place.");
      } else if (result?.code === "version_conflict") {
        const exactCacheGeneration = this.publicCacheGeneration;
        const contextCurrent = () => (
          identityGeneration === this.identityGeneration
          && this.readerCell?.id === cell.id
          && readerGeneration === this.readerGeneration
          && exactCacheGeneration === this.publicCacheGeneration
        );
        const latest = await this.readAuthoritativeCell(cell.id, contextCurrent);
        if (latest?.rebuildRequired) {
          await this.rebuildPublicProjection(latest.rebuild);
        } else if (latest?.cell && contextCurrent()) {
          this.mergeCell(latest.cell);
          this.openReader(latest.cell);
        }
        this.toast("That comb changed on another screen. It was not removed; review the latest version first.");
      } else {
        this.toast(result?.message || "The wall did not confirm that removal. Nothing was changed.");
      }
    } catch (error) {
      this.toast(`Could not remove that comb: ${error.message}`);
    } finally {
      if (identityGeneration === this.identityGeneration && readerGeneration === this.readerGeneration) this.setReaderBusy(false);
    }
  }

  async submitReport() {
    if (this.readerBusy) return;
    const cell = this.readerCell;
    if (!cell) return;
    if (this.bounds.readOnly) {
      this.toast("The Wall is open to read while Hive Watch is verified. Your selected report reason is still here.");
      return;
    }
    const reason = this.root.querySelector("[data-wall-report-reason]").value;
    if (!["personal_attack", "illegal", "spam", "privacy"].includes(reason)) {
      this.toast("Choose why the Hive watch should review this comb.");
      this.root.querySelector("[data-wall-report-reason]").focus();
      return;
    }
    const readerGeneration = this.readerGeneration;
    const identityGeneration = this.identityGeneration;
    this.setReaderBusy(true);
    if (!this.reportRequest
      || this.reportRequest.cellId !== cell.id
      || this.reportRequest.contentVersion !== cell.contentVersion
      || this.reportRequest.reason !== reason) {
      this.reportRequest = {
        requestId: randomRequestId(),
        cellId: cell.id,
        contentVersion: cell.contentVersion,
        reason,
      };
    }
    const reportRequest = this.reportRequest;
    try {
      const result = await this.backend.wallReportCell({
        requestId: reportRequest.requestId,
        cellId: cell.id,
        expectedContentVersion: cell.contentVersion,
        reason,
      });
      if (identityGeneration !== this.identityGeneration
        || this.readerCell?.id !== cell.id || readerGeneration !== this.readerGeneration) return;
      if (this.reportRequest === reportRequest) this.reportRequest = null;
      if (!result?.ok) {
        if (result?.code === "content_conflict") {
          const exactCacheGeneration = this.publicCacheGeneration;
          const contextCurrent = () => (
            identityGeneration === this.identityGeneration
            && this.readerCell?.id === cell.id
            && readerGeneration === this.readerGeneration
            && exactCacheGeneration === this.publicCacheGeneration
          );
          const latest = await this.readAuthoritativeCell(cell.id, contextCurrent);
          if (latest?.rebuildRequired) {
            await this.rebuildPublicProjection(latest.rebuild);
          } else if (latest?.cell && contextCurrent()) {
            this.mergeCell(latest.cell);
            this.openReader(latest.cell);
          }
        }
        this.toast(result?.message || "The Hive watch did not confirm that report.");
        return;
      }
      const rebuild = this.captureProjectionRebuild();
      if (identityGeneration === this.identityGeneration && readerGeneration === this.readerGeneration) this.setReaderBusy(false);
      this.closeDialog(this.reader);
      this.readerCell = null;
      await this.rebuildAfterConfirmedWrite(rebuild);
      this.toast("Your report went privately to the Hive watch.");
    } catch (error) {
      this.toast(`Could not send that report: ${error.message}`);
    } finally {
      if (identityGeneration === this.identityGeneration && readerGeneration === this.readerGeneration) this.setReaderBusy(false);
    }
  }

  async probeOperator() {
    const button = this.root.querySelector('[data-wall-action="watch"]');
    if (!button || typeof this.backend.wallOperatorStatus !== "function") return false;
    const probeGeneration = ++this.operatorProbeGeneration;
    const activationGeneration = this.activationGeneration;
    const identityGeneration = this.identityGeneration;
    try {
      const result = await this.backend.wallOperatorStatus();
      if (probeGeneration !== this.operatorProbeGeneration
        || !this.isContextCurrent(activationGeneration, identityGeneration)) return false;
      this.operator = result?.operator || null;
      button.hidden = !this.operator;
      if (this.operator) button.textContent = result?.cases?.length ? "Hive watch · review" : "Hive watch";
      const readerWatch = this.root.querySelector('[data-wall-reader-action="watch-open"]');
      if (readerWatch) readerWatch.hidden = !this.operator || this.readerCell?.status !== "visible";
      return Boolean(this.operator);
    } catch {
      if (probeGeneration !== this.operatorProbeGeneration
        || !this.isContextCurrent(activationGeneration, identityGeneration)) return false;
      this.operator = null;
      button.hidden = true;
      return false;
    }
  }

  async submitOperatorCase() {
    if (this.readerBusy) return;
    const cell = this.readerCell;
    if (!cell || !this.operator || typeof this.backend.wallOpenCase !== "function") return;
    const reasonField = this.root.querySelector("[data-wall-operator-reason]");
    const reason = reasonField.value.trim();
    if (reason.length < 3) {
      this.toast("Write a short evidence-based reason for opening the case.");
      reasonField.focus();
      return;
    }
    if (!window.confirm(`Open an urgent private review case for comb ${cell.q}, ${cell.r} by ${cell.author || "this author"}?`)) return;
    const activationGeneration = this.activationGeneration;
    const identityGeneration = this.identityGeneration;
    const readerGeneration = this.readerGeneration;
    if (!this.operatorCaseRequest
      || this.operatorCaseRequest.cellId !== cell.id
      || this.operatorCaseRequest.expectedVersion !== cell.version
      || this.operatorCaseRequest.reason !== reason) {
      this.operatorCaseRequest = {
        requestId: randomRequestId(),
        cellId: cell.id,
        expectedVersion: cell.version,
        reason,
      };
    }
    const operatorRequest = this.operatorCaseRequest;
    this.setReaderBusy(true);
    try {
      const result = await this.backend.wallOpenCase({
        requestId: operatorRequest.requestId,
        cellId: cell.id,
        expectedVersion: cell.version,
        reason,
        priority: "urgent",
      });
      if (!this.isContextCurrent(activationGeneration, identityGeneration)
        || readerGeneration !== this.readerGeneration || this.readerCell?.id !== cell.id) return;
      if (!result?.ok) {
        this.toast("That comb changed before the case opened. Review the current version first.");
        return;
      }
      if (this.operatorCaseRequest === operatorRequest) this.operatorCaseRequest = null;
      this.setReaderBusy(false);
      this.closeDialog(this.reader, { restoreFocus: false });
      const watchOpened = await this.openWatch();
      if (!this.isContextCurrent(activationGeneration, identityGeneration)) return;
      if (!watchOpened) return;
      await this.openWatchCase(result.cellId, result.contentVersion);
      this.toast("Urgent review case opened for the Hive watch.");
    } catch (error) {
      if (this.isContextCurrent(activationGeneration, identityGeneration)) this.toast(`The private case was not confirmed: ${error.message}`);
    } finally {
      if (this.isContextCurrent(activationGeneration, identityGeneration)
        && readerGeneration === this.readerGeneration) this.setReaderBusy(false);
    }
  }

  async openWatch() {
    const generation = ++this.watchGeneration;
    this.watchQueueRequestGeneration += 1;
    this.watchCaseRequestGeneration += 1;
    this.watchActionGeneration += 1;
    const activationGeneration = this.activationGeneration;
    const identityGeneration = this.identityGeneration;
    if (!this.operator && !(await this.probeOperator())) {
      if (generation !== this.watchGeneration
        || !this.isContextCurrent(activationGeneration, identityGeneration)) return false;
      this.toast("This saved account is not on the Hive watch allowlist.");
      return false;
    }
    if (generation !== this.watchGeneration
      || !this.isContextCurrent(activationGeneration, identityGeneration)) return false;
    this.clearWatchDom();
    this.watchCases = [];
    this.watchCursor = null;
    this.watchHasMore = false;
    this.showDialog(this.watch, this.watch.querySelector("[data-wall-close='watch']"));
    await this.loadWatchQueue(true, generation);
    if (generation !== this.watchGeneration
      || !this.isContextCurrent(activationGeneration, identityGeneration)) return false;
    if (this.watchPollTimer) window.clearInterval(this.watchPollTimer);
    this.watchPollTimer = window.setInterval(() => {
      if (this.watch.open && !this.watchBusy && generation === this.watchGeneration) {
        void this.loadWatchQueue(true, generation, true);
      }
    }, 15_000);
    return true;
  }

  async loadWatchQueue(reset = false, watchGeneration = this.watchGeneration, quiet = false) {
    if (watchGeneration !== this.watchGeneration || this.watchBusy) return false;
    const requestGeneration = ++this.watchQueueRequestGeneration;
    this.watchLoadingGeneration = requestGeneration;
    const activationGeneration = this.activationGeneration;
    const identityGeneration = this.identityGeneration;
    const cursor = reset ? null : this.watchCursor;
    const status = this.root.querySelector("[data-wall-watch-status]");
    const priorFingerprint = this.watchQueueFingerprint();
    if (!quiet) setTextIfChanged(status, reset ? "Loading attributable review cases…" : "Loading more review cases…");
    try {
      const result = await this.backend.wallModerationQueue(cursor, 50);
      if (watchGeneration !== this.watchGeneration
        || requestGeneration !== this.watchQueueRequestGeneration
        || !this.isContextCurrent(activationGeneration, identityGeneration)) return false;
      this.operator = result?.operator || this.operator;
      const cases = Array.isArray(result?.cases) ? result.cases : [];
      if (reset) this.watchCases = [];
      const known = new Set(this.watchCases.map((item) => `${item.cellId}:${item.contentVersion}`));
      for (const item of cases) {
        const key = `${item.cellId}:${item.contentVersion}`;
        if (!known.has(key)) {
          this.watchCases.push(item);
          known.add(key);
        }
      }
      this.watchCursor = result?.nextCursor || null;
      this.watchHasMore = Boolean(result?.hasMore && this.watchCursor);
      const changed = priorFingerprint !== this.watchQueueFingerprint();
      if (!quiet || changed) {
        this.renderWatchQueue();
        setTextIfChanged(status, `${this.operator?.name || "Hive watch"} · ${this.operator?.role || "reviewer"} · ${this.watchCases.length}${this.watchHasMore ? "+" : ""} active cases`);
      }
      return true;
    } catch (error) {
      if (watchGeneration === this.watchGeneration
        && requestGeneration === this.watchQueueRequestGeneration
        && this.isContextCurrent(activationGeneration, identityGeneration) && !quiet) {
        setTextIfChanged(status, `The private review queue could not load: ${error.message}`);
      }
      return false;
    } finally {
      if (this.watchLoadingGeneration === requestGeneration) this.watchLoadingGeneration = 0;
    }
  }

  watchQueueFingerprint() {
    return JSON.stringify({
      operator: [this.operator?.name || "", this.operator?.role || ""],
      cursor: this.watchCursor || null,
      hasMore: Boolean(this.watchHasMore),
      cases: this.watchCases.map((item) => [
        item.cellId,
        Number(item.contentVersion) || 0,
        item.priority || "",
        Number(item.reportCount) || 0,
        Number(item.q) || 0,
        Number(item.r) || 0,
        item.caseStatus || "",
        item.currentStatus || "",
      ]),
    });
  }

  renderWatchQueue() {
    const activeElement = globalThis.document?.activeElement || null;
    const focusedKey = activeElement && this.watch.contains(activeElement)
      ? activeElement.dataset?.wallWatchKey || activeElement.dataset?.wallWatchFocus || null
      : null;
    const focusedTop = focusedKey && typeof activeElement.getBoundingClientRect === "function"
      ? activeElement.getBoundingClientRect().top
      : null;
    const existing = new Map([...this.watchQueue.querySelectorAll("[data-wall-watch-key]")]
      .map((button) => [button.dataset.wallWatchKey, button]));
    const desiredKeys = new Set();
    let cursor = this.watchQueue.firstElementChild;
    for (const item of this.watchCases) {
      const key = `${item.cellId}:${item.contentVersion}`;
      desiredKeys.add(key);
      let button = existing.get(key);
      if (!button) {
        button = document.createElement("button");
        button.type = "button";
        button.className = "honey-watch-case";
        button.dataset.wallWatchKey = key;
        const title = document.createElement("strong");
        title.dataset.wallWatchPart = "title";
        const detail = document.createElement("span");
        detail.dataset.wallWatchPart = "detail";
        button.append(title, detail);
      }
      button.dataset.wallWatchCase = item.cellId;
      button.dataset.wallWatchVersion = String(item.contentVersion);
      const selected = this.watchCase?.case?.cellId === item.cellId
        && Number(this.watchCase?.case?.contentVersion) === Number(item.contentVersion);
      button.setAttribute("aria-controls", "honeyWatchDetail");
      button.setAttribute("aria-expanded", String(selected));
      if (selected) button.setAttribute("aria-current", "true");
      else button.removeAttribute("aria-current");
      setTextIfChanged(button.querySelector('[data-wall-watch-part="title"]'), `${item.priority === "urgent" ? "Urgent" : "Normal"} · ${item.reportCount} ${item.reportCount === 1 ? "report" : "reports"}`);
      setTextIfChanged(button.querySelector('[data-wall-watch-part="detail"]'), `Comb ${item.q}, ${item.r} · ${item.caseStatus} · ${item.currentStatus}`);
      if (button !== cursor) this.watchQueue.insertBefore(button, cursor);
      cursor = button.nextElementSibling;
    }
    for (const [key, button] of existing) {
      if (!desiredKeys.has(key)) button.remove();
    }
    let empty = this.watchQueue.querySelector("[data-wall-watch-empty]");
    if (!this.watchCases.length) {
      if (!empty) {
        empty = document.createElement("p");
        empty.className = "honey-list-empty";
        empty.dataset.wallWatchEmpty = "true";
        this.watchQueue.append(empty);
      }
      setTextIfChanged(empty, "No Wall cases need a human decision.");
    } else empty?.remove();
    const more = this.root.querySelector("[data-wall-watch-more]");
    more.hidden = !this.watchHasMore;
    more.disabled = this.watchBusy;
    if (focusedKey) {
      const focusTarget = (focusedKey === "more" && !more.hidden && !more.disabled ? more : null)
        || this.watchQueue.querySelector(`[data-wall-watch-key="${escapeSelectorValue(focusedKey)}"]`)
        || this.watchQueue.querySelector("[data-wall-watch-key]")
        || (!more.hidden && !more.disabled ? more : this.watch.querySelector("[data-wall-close='watch']"));
      const restore = () => {
        focusTarget?.focus?.({ preventScroll: true });
        if (focusTarget && focusedTop != null && typeof globalThis.window?.scrollBy === "function") {
          const topDelta = focusTarget.getBoundingClientRect().top - focusedTop;
          if (topDelta) globalThis.window.scrollBy({ top: topDelta, behavior: "auto" });
        }
      };
      if (typeof globalThis.requestAnimationFrame === "function") requestAnimationFrame(restore);
      else restore();
    }
  }

  async openWatchCase(cellId, contentVersion) {
    if (this.watchBusy) return false;
    const watchGeneration = this.watchGeneration;
    const requestGeneration = ++this.watchCaseRequestGeneration;
    const activationGeneration = this.activationGeneration;
    const identityGeneration = this.identityGeneration;
    const status = this.root.querySelector("[data-wall-watch-status]");
    this.watchCase = null;
    this.watchDetail.hidden = true;
    this.watchReason.value = "";
    status.textContent = "Loading preserved evidence…";
    try {
      const detail = await this.backend.wallModerationDetail(cellId, contentVersion);
      if (watchGeneration !== this.watchGeneration
        || requestGeneration !== this.watchCaseRequestGeneration
        || !this.isContextCurrent(activationGeneration, identityGeneration)) return false;
      if (!detail?.case || !detail?.current) throw new Error("That review case is no longer available.");
      this.watchCase = detail;
      this.watchActionRequest = null;
      this.renderWatchCase();
      this.watchDetail.hidden = false;
      this.renderWatchQueue();
      this.watchDetail.scrollIntoView({ block: "nearest", behavior: this.motionBehavior("smooth") });
      this.root.querySelector("#honeyWatchCaseTitle")?.focus?.({ preventScroll: true });
      status.textContent = `${detail.operator?.name || "Hive watch"} · evidence is private to allowlisted reviewers`;
      return true;
    } catch (error) {
      if (watchGeneration === this.watchGeneration
        && requestGeneration === this.watchCaseRequestGeneration
        && this.isContextCurrent(activationGeneration, identityGeneration)) status.textContent = `That case could not load: ${error.message}`;
      return false;
    }
  }

  renderWatchCase() {
    const detail = this.watchCase;
    if (!detail?.case || !detail?.current) return;
    const moderationCase = detail.case;
    const current = detail.current;
    this.root.querySelector("[data-wall-watch-meta]").textContent = `${moderationCase.priority} priority · ${moderationCase.status} · comb ${current.q}, ${current.r} · content v${moderationCase.contentVersion} · current v${current.version}`;
    this.root.querySelector("[data-wall-watch-evidence]").textContent = `${moderationCase.evidenceAuthor || "Hive member"}: ${moderationCase.evidenceBody}`;
    this.root.querySelector("[data-wall-watch-current]").textContent = current.contentVersion === moderationCase.contentVersion
      ? `Current public version: ${current.body || "No public body is visible."}`
      : `Current public content is now version ${current.contentVersion}: ${current.body || "No public body is visible."}`;
    const reportSummary = this.root.querySelector("[data-wall-watch-reports]");
    reportSummary.replaceChildren();
    const reports = Array.isArray(detail.reports) ? detail.reports : [];
    const summary = document.createElement("p");
    summary.textContent = reports.length
      ? `Private reports: ${reports.map((report) => `${report.reporter || "verified reporter"} (${String(report.reason).replaceAll("_", " ")})`).join(", ")}`
      : "Operator-opened case; no visitor report is attached.";
    reportSummary.append(summary);
    const eventSummary = this.root.querySelector("[data-wall-watch-events]");
    eventSummary.replaceChildren();
    const events = Array.isArray(detail.events) ? detail.events : [];
    const history = document.createElement("p");
    history.textContent = events.length
      ? `Case history: ${events.map((event) => `${event.operator || "Hive watch"} ${String(event.action).replaceAll("_", " ")}`).join("; ")}`
      : "No prior operator action is recorded for this case.";
    eventSummary.append(history);
    const actions = this.root.querySelector("[data-wall-watch-actions]");
    actions.replaceChildren();
    const addAction = (action, label, primary = false) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = `honey-tool${primary ? " honey-tool-primary" : ""}`;
      button.dataset.wallWatchAction = action;
      button.textContent = label;
      actions.append(button);
    };
    if (moderationCase.status === "contained") addAction("restore", "Restore publicly");
    else {
      addAction("dismiss", "Dismiss report");
      if (current.status === "visible" && current.contentVersion === moderationCase.contentVersion) addAction("quarantine", "Contain for review", true);
    }
    if (this.operator?.role === "admin" && ["visible", "quarantined"].includes(current.status)) addAction("remove", "Remove globally");
  }

  async moderateWatchCase(action) {
    if (this.watchBusy) return;
    const detail = this.watchCase;
    if (!detail?.case || !detail?.current) return;
    const reason = this.root.querySelector("[data-wall-watch-reason]").value.trim();
    if (reason.length < 3) {
      this.root.querySelector("[data-wall-watch-status]").textContent = "Write a short evidence-based decision note first.";
      return;
    }
    const actionLabel = { quarantine: "Contain for review", dismiss: "Dismiss this case", restore: "Restore publicly", remove: "Remove globally" }[action] || action;
    const current = detail.current;
    if (!window.confirm(`${actionLabel} for comb ${current.q}, ${current.r} by ${current.author || "this author"}? This exact action and your note will be audited.`)) return;
    const watchGeneration = this.watchGeneration;
    const caseRequestGeneration = this.watchCaseRequestGeneration;
    const actionGeneration = ++this.watchActionGeneration;
    const activationGeneration = this.activationGeneration;
    const identityGeneration = this.identityGeneration;
    const fingerprint = [
      detail.case.cellId,
      detail.case.contentVersion,
      detail.case.generation,
      detail.current.version,
      action,
      reason,
    ].join(":");
    if (!this.watchActionRequest || this.watchActionRequest.fingerprint !== fingerprint) {
      this.watchActionRequest = { requestId: randomRequestId(), fingerprint };
    }
    const watchRequest = this.watchActionRequest;
    this.watchBusy = true;
    this.setWatchLocked(true);
    try {
      const result = await this.backend.wallModerateCase({
        requestId: watchRequest.requestId,
        cellId: detail.case.cellId,
        contentVersion: detail.case.contentVersion,
        caseGeneration: detail.case.generation,
        expectedVersion: detail.current.version,
        action,
        reason,
      });
      if (watchGeneration !== this.watchGeneration
        || caseRequestGeneration !== this.watchCaseRequestGeneration
        || actionGeneration !== this.watchActionGeneration
        || !this.isContextCurrent(activationGeneration, identityGeneration)) return;
      if (!result?.ok) {
        this.root.querySelector("[data-wall-watch-status]").textContent = "That case changed on another screen. Reloading current evidence…";
        this.watchBusy = false;
        this.setWatchLocked(false);
        await this.openWatchCase(detail.case.cellId, detail.case.contentVersion);
        return;
      }
      if (this.watchActionRequest === watchRequest) this.watchActionRequest = null;
      const rebuild = this.captureProjectionRebuild();
      this.watchCase = null;
      this.watchDetail.hidden = true;
      this.watchReason.value = "";
      this.watchBusy = false;
      this.setWatchLocked(false);
      await this.rebuildAfterConfirmedWrite(rebuild);
      await this.loadWatchQueue(true, watchGeneration);
      const focusTarget = this.watchQueue.querySelector("[data-wall-watch-key]")
        || (!this.root.querySelector("[data-wall-watch-more]")?.hidden ? this.root.querySelector("[data-wall-watch-more]") : null)
        || this.watch.querySelector('[data-wall-close="watch"]');
      focusTarget?.focus?.({ preventScroll: true });
      this.toast(`Hive watch recorded: ${action}.`);
    } catch (error) {
      if (watchGeneration === this.watchGeneration
        && actionGeneration === this.watchActionGeneration
        && this.isContextCurrent(activationGeneration, identityGeneration)) {
        this.root.querySelector("[data-wall-watch-status]").textContent = `The action was not confirmed: ${error.message}`;
      }
    } finally {
      if (watchGeneration === this.watchGeneration && actionGeneration === this.watchActionGeneration) {
        this.watchBusy = false;
        this.setWatchLocked(false);
      }
    }
  }

  setWatchLocked(locked) {
    for (const control of this.watch.querySelectorAll("button, textarea")) {
      control.disabled = Boolean(locked) && !control.matches('[data-wall-close="watch"]');
    }
    if (!locked) this.renderWatchQueue();
  }

  showDialog(dialog, focusTarget, openerOverride = null) {
    const wasOpen = Boolean(dialog.open || dialog.hasAttribute("open"));
    if (!wasOpen) {
      const active = document.activeElement;
      const opener = openerOverride || (active instanceof HTMLElement ? active : null);
      this.dialogOpeners.set(dialog, opener);
      const focusKey = opener?.dataset?.wallListFocus || null;
      this.dialogFocusAnchors.set(dialog, focusKey ? {
        focusKey,
        focusTop: typeof opener.getBoundingClientRect === "function" ? opener.getBoundingClientRect().top : null,
      } : null);
    }
    if (typeof dialog.showModal === "function") {
      if (!dialog.open) dialog.showModal();
    } else {
      dialog.setAttribute("open", "");
    }
    if (this.focusFrame) cancelAnimationFrame(this.focusFrame);
    this.focusFrame = requestAnimationFrame(() => {
      this.focusFrame = null;
      focusTarget?.focus();
    });
  }

  closeDialog(dialog, options = {}) {
    if (!dialog) return null;
    if (dialog === this.compose && this.composeState) {
      const persisted = this.composeState.editing ? this.saveEditDraft() : this.saveDraft();
      if (persisted === false) {
        const closeAnyway = (dialog === this.compose && this.composeBusy)
          || window.confirm("This browser cannot save the draft outside this tab. Close the composer anyway? Your words remain here until this page reloads.");
        if (!closeAnyway) return null;
        this.toast("The draft remains only in this tab. Copy it before reloading or switching accounts.");
      }
    }
    if (dialog === this.compose) {
      this.composeGeneration += 1;
      this.composeBusy = false;
      this.composeBody.readOnly = false;
      this.composeState = null;
      this.updateResumeDraftAction();
    }
    if (dialog === this.reader) {
      this.readerGeneration += 1;
      this.readerBusy = false;
      this.readerCell = null;
      this.reportRequest = null;
      this.operatorCaseRequest = null;
    }
    if (dialog === this.watch) {
      this.watchGeneration += 1;
      this.watchQueueRequestGeneration += 1;
      this.watchCaseRequestGeneration += 1;
      this.watchActionGeneration += 1;
      if (this.watchPollTimer) window.clearInterval(this.watchPollTimer);
      this.watchPollTimer = null;
      this.watchCase = null;
      this.watchBusy = false;
      this.clearWatchDom();
    }
    const restoreFocus = options.restoreFocus !== false;
    const opener = this.dialogOpeners.get(dialog);
    const focusAnchor = this.dialogFocusAnchors.get(dialog);
    if (typeof dialog.close === "function" && dialog.open) dialog.close();
    else dialog.removeAttribute("open");
    this.dialogOpeners.delete(dialog);
    this.dialogFocusAnchors.delete(dialog);
    if (!restoreFocus) return opener;
    this.restoreWallFocus(opener, focusAnchor);
    return opener;
  }

  restoreWallFocus(opener = null, focusAnchor = null) {
    if (this.focusFrame) cancelAnimationFrame(this.focusFrame);
    this.focusFrame = requestAnimationFrame(() => {
      this.focusFrame = null;
      if (opener?.isConnected && !opener.disabled && opener.getAttribute?.("aria-disabled") !== "true"
        && !opener.closest("[hidden]") && !opener.closest("dialog:not([open])")) {
        opener.focus({ preventScroll: true });
        if (!globalThis.document || globalThis.document.activeElement === opener) return;
      }
      if (this.mode === "list") {
        const keyedCandidate = focusAnchor?.focusKey
          ? this.list?.querySelector(`[data-wall-list-focus="${escapeSelectorValue(focusAnchor.focusKey)}"]`)
          : null;
        const keyedTarget = keyedCandidate && !keyedCandidate.hidden && !keyedCandidate.disabled
          && !keyedCandidate.closest?.("[hidden]") ? keyedCandidate : null;
        const listTarget = keyedTarget || this.list?.querySelector("button:not([hidden])") || this.list;
        listTarget?.focus({ preventScroll: true });
        if (listTarget && focusAnchor?.focusTop != null && typeof window.scrollBy === "function") {
          const topDelta = listTarget.getBoundingClientRect().top - focusAnchor.focusTop;
          if (topDelta) window.scrollBy({ top: topDelta, behavior: "auto" });
        }
      } else {
        this.stage?.focus({ preventScroll: true });
      }
    });
  }

  showComposeError(message, suggestion = null) {
    this.composeError.hidden = false;
    this.composeError.textContent = message;
    this.composeBody?.setAttribute?.("aria-invalid", "true");
    const button = this.root.querySelector("[data-wall-compose-suggestion]");
    if (suggestion?.editConflict) {
      button.textContent = "Review latest comb";
      button.hidden = false;
    } else if (suggestion && Number.isInteger(suggestion.q) && Number.isInteger(suggestion.r)) {
      if (this.composeState) this.composeState.suggestedCoordinate = { q: suggestion.q, r: suggestion.r };
      button.textContent = `Move draft to ${suggestion.q}, ${suggestion.r}`;
      button.hidden = false;
    } else {
      if (this.composeState) this.composeState.suggestedCoordinate = null;
      button.hidden = true;
    }
  }

  hideComposeError() {
    this.composeError.hidden = true;
    this.composeError.textContent = "";
    this.composeBody?.removeAttribute?.("aria-invalid");
    const button = this.root.querySelector("[data-wall-compose-suggestion]");
    button.hidden = true;
    button.textContent = "Use highlighted comb";
    if (this.composeState) this.composeState.suggestedCoordinate = null;
  }

  saveDraft() {
    if (this.composeState?.editing) return;
    if (!this.composeState && !this.draft) return;
    const source = this.composeState && !this.composeState.editing ? this.composeState : this.draft;
    if (!source) return;
    const draft = {
      q: Number(source.q),
      r: Number(source.r),
      body: this.composeBody && this.compose.open ? this.composeBody.value : source.body || "",
      requestId: source.requestId || null,
      conversationId: source.conversationId || null,
      replyToId: source.replyToId || null,
      intent: PLACEMENT_INTENTS.has(source.intent) ? source.intent : null,
      anchorCellIds: canonicalAnchorIds(source.anchorCellIds),
      choiceMade: source.choiceMade !== false,
      choiceExplicit: source.choiceExplicit === true,
      requiresConversationChoice: source.requiresConversationChoice === true,
      mode: source.mode || null,
      updatedAt: Date.now(),
    };
    this.draft = draft;
    const key = this.scopedStorageKey(DRAFT_KEY);
    if (!key || !safeSet(this.storage, key, draft)) {
      this.draftPersistenceFailed = true;
      this.showComposeError("This browser cannot persist the draft. It is still open on this page, but reloading or switching accounts could lose it.");
      this.updateResumeDraftAction();
      return false;
    }
    this.draftPersistenceFailed = false;
    this.updateResumeDraftAction();
    return true;
  }

  clearDraft() {
    const key = this.scopedStorageKey(DRAFT_KEY);
    this.draft = null;
    this.composeState = null;
    if (this.draftFromHandoff) {
      try { this.handoffStorage.removeItem(DRAFT_HANDOFF_KEY); } catch { /* no-op */ }
    }
    this.draftFromHandoff = false;
    if (key) {
      try { this.storage.removeItem(key); } catch { /* no-op */ }
    }
    this.updateResumeDraftAction();
  }

  saveEditDraft() {
    const state = this.composeState?.editing ? this.composeState : this.editDraft;
    if (!state?.cellId) return;
    const draft = {
      cellId: state.cellId,
      expectedVersion: Number(state.expectedVersion) || 1,
      q: Number(state.q),
      r: Number(state.r),
      body: this.composeState?.editing && this.composeBody && this.compose.open ? this.composeBody.value : state.body || "",
      requestId: state.requestId || null,
      updatedAt: Date.now(),
    };
    this.editDraft = draft;
    const key = this.scopedStorageKey(EDIT_DRAFT_KEY);
    if (!key || !safeSet(this.storage, key, draft)) {
      this.editDraftPersistenceFailed = true;
      this.showComposeError("This browser cannot persist the edit draft. It is still open on this page, but reloading or switching accounts could lose it.");
      return false;
    }
    this.editDraftPersistenceFailed = false;
    return true;
  }

  clearEditDraft() {
    const key = this.scopedStorageKey(EDIT_DRAFT_KEY);
    this.editDraft = null;
    this.editDraftPersistenceFailed = false;
    if (key) {
      try { this.storage.removeItem(key); } catch { /* no-op */ }
    }
  }

  markSeen(cellId) {
    if (!cellId) return;
    this.seen.add(cellId);
    if (this.seen.size > 800) this.seen = new Set([...this.seen].slice(-600));
    const key = this.scopedStorageKey(SEEN_KEY);
    if (key) safeSet(this.storage, key, [...this.seen]);
    this.updateNewCount();
    const node = this.root.querySelector(`#honey-cell-${CSS.escape(cellId)}`);
    node?.classList.remove("is-unseen");
  }

  updateNewCount() {
    if (!this.newCount) return;
    const ids = new Set([...this.cellsById.keys(), ...this.pendingEvents.keys()]);
    const count = [...ids].filter((id) => !this.seen.has(id)).length;
    this.newCount.hidden = count === 0;
    this.newCount.textContent = count ? String(Math.min(count, 99)) : "";
  }

  findNew() {
    const unseen = [...this.cells.values()].filter((cell) => !this.seen.has(cell.id)).sort((a, b) => b.seq - a.seq)[0];
    if (!unseen) {
      this.toast("No unread combs in the loaded wall yet.");
      return;
    }
    this.openReader(unseen);
  }

  findMine() {
    const mine = [...this.cells.values()].filter((cell) => cell.isMine).sort((a, b) => b.seq - a.seq)[0];
    if (!mine) {
      this.toast(this.getUser() ? "No combs from you are loaded here yet." : "Create a profile to find your combs.");
      if (!this.getUser()) this.requestIdentity({ reason: "wall-mine" });
      return;
    }
    this.openReader(mine);
  }

  // Zoom about a fixed stage point so the thing under the cursor, the pinch
  // midpoint, or the selected comb stays put. Without an anchor the wall
  // slides out from under the viewer on every step.
  setZoom(value, anchor = null, options = {}) {
    const previousZoom = this.zoom;
    const nextZoom = this.clampZoom(value);
    const size = this.stageSize();
    const point = anchor && Number.isFinite(anchor.x) && Number.isFinite(anchor.y)
      ? anchor
      : { x: size.width / 2, y: size.height / 2 };
    const before = this.stageToWorld(point.x, point.y);
    this.zoom = nextZoom;
    this.cameraX = point.x - size.width / 2 - before.x * nextZoom;
    this.cameraY = point.y - (this.world.originY + before.y) * nextZoom;
    this.clampCamera();
    this.updatePlaneTransform();
    this.updateTarget();
    this.scheduleViewport();
    if (options.announce && nextZoom !== previousZoom) {
      this.announce(`Wall zoom ${Math.round(nextZoom * 100)} percent.`);
    }
    return this.zoom;
  }

  // "See it all": pull back until the whole occupied wall is inside the stage.
  fitWall(announce = false) {
    this.cancelGrowthIntent({ restoreFocus: false, announce: false });
    this.ensureStageOnScreen();
    const content = this.contentExtent();
    this.zoom = this.clampZoom(this.fitZoom());
    const camera = this.cameraForFocus(
      (content.minX + content.maxX) / 2,
      (content.minY + content.maxY) / 2,
    );
    this.cameraX = camera.x;
    this.cameraY = camera.y;
    this.clampCamera();
    this.updatePlaneTransform();
    this.updateTarget();
    this.scheduleViewport();
    if (announce) {
      this.announce(`The whole Hive is in view at ${Math.round(this.zoom * 100)} percent zoom. ${this.bounds.messageCount} combs.`);
    }
  }

  frameInitialPhoneView() {
    if (!this.stage || this.stage.hidden) return;
    const extent = this.initialPhoneExtent();
    this.zoom = this.clampZoom(Math.min(1, this.zoomToFit(extent)));
    const camera = this.cameraForFocus(
      (extent.minX + extent.maxX) / 2,
      (extent.minY + extent.maxY) / 2,
      this.zoom,
    );
    this.cameraX = camera.x;
    this.cameraY = camera.y;
    this.clampCamera();
    this.updatePlaneTransform();
    this.updateTarget();
    this.scheduleViewport();
  }

  // The stage is a fixed box inside a page that still scrolls past a header,
  // so a deliberate wall move must first make sure the map is actually on
  // screen. Only fires when most of the stage is out of view, which keeps it
  // from fighting the scroll listener that re-runs the growth fit.
  ensureStageOnScreen() {
    const view = globalThis.window;
    if (!this.stage || this.stage.hidden || typeof view?.scrollTo !== "function") return;
    const rect = this.stage.getBoundingClientRect?.();
    const viewportHeight = Number(view.visualViewport?.height) || view.innerHeight || 0;
    if (!rect?.height || !viewportHeight) return;
    const visible = Math.min(rect.bottom, viewportHeight) - Math.max(rect.top, 0);
    if (visible >= Math.min(rect.height, viewportHeight) * 0.9) return;
    const slack = Math.max(0, (viewportHeight - rect.height) / 2);
    view.scrollTo({ top: Math.max(0, rect.top + (Number(view.scrollY) || 0) - slack), behavior: "auto" });
  }

  panBy(dx, dy) {
    this.cameraX -= Number(dx) || 0;
    this.cameraY -= Number(dy) || 0;
    this.clampCamera();
    this.updatePlaneTransform();
    this.updateTarget();
    this.scheduleViewport();
  }

  centerWall(announce = false) {
    this.cancelGrowthIntent({ restoreFocus: false, announce: false });
    this.selected = { q: 0, r: 0 };
    this.revealCoordinate(0, 0);
    if (announce) this.announce("Centered on the first comb.");
  }

  motionBehavior(preferred = "smooth") {
    if (preferred !== "smooth") return preferred;
    return globalThis.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches ? "auto" : "smooth";
  }

  // Glide the camera so a coordinate lands in the middle of the stage. Reduced
  // motion drops the transition rather than the movement.
  revealCoordinate(q, r, behavior = "smooth") {
    this.selected = { q: Number(q), r: Number(r) };
    // Unconditional, including the "auto" reveal on activation: the scroll
    // model this replaced always brought the wall itself into the viewport,
    // and skipping it left phones looking at an empty stage below the fold.
    this.ensureStageOnScreen();
    const point = axialToPixel(q, r, this.hexRadius);
    const view = this.visibleStageRect();
    this.setCameraForWorldPoint(point, (view.left + view.right) / 2, (view.top + view.bottom) / 2, behavior);
    this.updateTarget();
    this.scheduleViewport();
  }

  setCameraForWorldPoint(point, stageX, stageY, behavior = "auto") {
    const size = this.stageSize();
    this.cameraX = stageX - size.width / 2 - point.x * this.zoom;
    this.cameraY = stageY - (this.world.originY + point.y) * this.zoom;
    this.clampCamera();
    this.applyCameraMotion(behavior);
    this.updatePlaneTransform();
  }

  // The plane animates its own transform instead of asking the document to
  // scroll. The class is removed on the next frame so drags stay immediate.
  applyCameraMotion(behavior = "auto") {
    if (!this.plane) return;
    if (this.motionBehavior(behavior) !== "smooth") {
      this.plane.classList.remove("is-gliding");
      return;
    }
    this.plane.classList.add("is-gliding");
    if (this.glideTimer) window.clearTimeout(this.glideTimer);
    this.glideTimer = window.setTimeout(() => {
      this.glideTimer = null;
      this.plane?.classList.remove("is-gliding");
      this.scheduleViewport();
    }, 340);
  }

  ensureCoordinateVisible(q, r, behavior = "smooth") {
    // While a growth picker is open its fit owns the camera and has already
    // placed all six directions on screen together. Nudging toward whichever
    // slot just took focus would push the opposite ones out of the viewport.
    if (this.growthIntent) {
      this.scheduleViewport();
      return;
    }
    this.ensureStageOnScreen();
    const point = axialToPixel(q, r, this.hexRadius);
    const view = this.visibleStageRect();
    const screen = this.worldToStage(point.x, point.y);
    const marginX = Math.min(180, view.width * 0.24);
    const marginY = Math.min(180, view.height * 0.24);
    const outsideX = screen.x < view.left + marginX || screen.x > view.right - marginX;
    const outsideY = screen.y < view.top + marginY || screen.y > view.bottom - marginY;
    if (!outsideX && !outsideY) {
      this.scheduleViewport();
      return;
    }
    // Only recentre the axis that actually ran off the edge; nudging both
    // makes single-step arrow navigation feel like the wall jumps.
    this.setCameraForWorldPoint(
      point,
      outsideX ? (view.left + view.right) / 2 : screen.x,
      outsideY ? (view.top + view.bottom) / 2 : screen.y,
      behavior,
    );
    this.updateTarget();
    this.scheduleViewport();
  }

  setMode(mode) {
    this.placementIntentGeneration += 1;
    this.focusGeneration += 1;
    const previousMode = this.mode;
    this.mode = mode === "list" ? "list" : "wall";
    try { this.storage.setItem(MODE_KEY, this.mode); } catch { /* no-op */ }
    if (this.mode === "list") {
      if (previousMode !== "list") this.invalidatePublicProjectionCache();
      this.applyMode();
      void this.refreshList(this.listCells.length === 0).finally(() => this.wakeListReconciliation());
    } else {
      if (previousMode === "list") this.invalidatePublicProjectionCache();
      else this.discardAccessibleListWindow();
      this.applyMode();
      this.scheduleViewport();
    }
  }

  discardAccessibleListWindow(resetConversation = true) {
    if (this.listReconcileTimer) window.clearTimeout(this.listReconcileTimer);
    this.listReconcileTimer = null;
    this.listReconcileEvents.clear();
    if (this.listItems) this.listItems.textContent = "Refreshing the authoritative Wall list…";
    this.listCells = [];
    this.listBeforeSeq = null;
    this.listHasMore = true;
    this.listWindowRevision = 0;
    if (resetConversation) this.listConversationId = null;
    this.listRequestGeneration += 1;
  }

  applyMode() {
    if (!this.wall) return;
    const listMode = this.mode === "list";
    this.stage.hidden = listMode;
    this.list.hidden = !listMode;
    this.root.querySelector("[data-wall-edge-note]").hidden = listMode;
    this.wall.dataset.wallMode = listMode ? "list" : "wall";
    for (const modeButton of this.root.querySelectorAll('[data-wall-action="mode"]')) {
      modeButton.textContent = listMode ? "Return to wall" : "Read as list";
      modeButton.setAttribute("aria-pressed", String(listMode));
    }
  }

  async refreshList(reset = false, preservedViewState = null) {
    const identityGeneration = this.identityGeneration;
    const cacheGeneration = this.publicCacheGeneration;
    const requestGeneration = ++this.listRequestGeneration;
    const conversationId = this.listConversationId;
    const viewState = preservedViewState || this.captureAccessibleListView(false);
    const priorCells = this.listCells.slice();
    const priorBeforeSeq = this.listBeforeSeq;
    const priorHasMore = this.listHasMore;
    let beforeSeq = reset ? null : priorBeforeSeq;
    let hasMore = reset ? true : priorHasMore;
    const incomingCells = [];
    let authoritativeListRevision = Number(this.listWindowRevision) || 0;
    const pageSize = conversationId ? 60 : 40;
    const pageBudget = 1;
    if (!hasMore) return;
    try {
      for (let page = 0; page < pageBudget && hasMore; page += 1) {
        const result = conversationId
          ? await this.backend.wallConversation(conversationId, beforeSeq, pageSize)
          : await this.backend.wallList(beforeSeq, pageSize);
        if (identityGeneration !== this.identityGeneration
          || cacheGeneration !== this.publicCacheGeneration
          || requestGeneration !== this.listRequestGeneration
          || conversationId !== this.listConversationId) return;
        if (!result && conversationId) throw new Error("That conversation is no longer available.");
        if (!result?.bounds || !Number.isSafeInteger(Number(result.bounds.revision))) {
          throw new Error("The wall list did not include an authoritative revision.");
        }
        const resultRevision = Number(result.bounds.revision);
        if (resultRevision < Math.max(this.bounds.revision, this.listWindowRevision)) {
          throw new Error("The wall list returned an older snapshot.");
        }
        const viewerAdvanced = this.viewerProjectionAdvanced(result.bounds);
        if (!this.applyAuthoritativeBounds(result.bounds)) throw new Error("The wall list returned an older snapshot.");
        if (viewerAdvanced && this.hasPublicProjection("list")) {
          const rebuildView = this.captureAccessibleListView(true);
          if (!rebuildView.focusKey && viewState.focusKey) Object.assign(rebuildView, viewState, { preserveScroll: true });
          this.invalidatePublicProjectionCache({ resetConversation: false });
          await this.refreshList(true, rebuildView);
          return;
        }
        const priorIds = new Set(priorCells.map((cell) => cell?.id).filter(Boolean));
        const accountedRevision = [...this.listReconcileEvents.values()].reduce((revision, event) => (
          priorIds.has(event?.cellId)
            ? Math.max(revision, Number(event?.revision) || 0)
            : revision
        ), Number(this.listWindowRevision) || 0);
        if (resultRevision > accountedRevision && (
          priorCells.length || this.cells?.size || this.conversations?.size || this.readerCell
        )) {
          // The list observed a revision that no known realtime invalidation
          // advanced. Some retained older row may have changed, so remove the
          // whole visible cache synchronously and rebuild from page one. A
          // failed rebuild leaves only the refresh status, never stale text.
          const rebuildView = this.captureAccessibleListView(true);
          if (!rebuildView.focusKey && viewState.focusKey) Object.assign(rebuildView, viewState, { preserveScroll: true });
          this.invalidatePublicProjectionCache({ resetConversation: false });
          await this.refreshList(true, rebuildView);
          return;
        }
        authoritativeListRevision = resultRevision;
        if (result?.conversation?.id) this.conversations.set(result.conversation.id, result.conversation);
        const incoming = Array.isArray(result?.cells) ? result.cells : [];
        for (const cell of incoming) {
          const canonical = this.mergeCell(cell, { renderList: false });
          if (canonical) {
            incomingCells.push(canonical);
            this.listReconcileEvents.delete(canonical.id);
          }
        }
        const nextBeforeSeq = result?.nextBeforeSeq || null;
        hasMore = Boolean(result?.hasMore);
        if (!incoming.length || nextBeforeSeq === beforeSeq) {
          beforeSeq = nextBeforeSeq;
          break;
        }
        beforeSeq = nextBeforeSeq;
      }
      if (identityGeneration !== this.identityGeneration
        || cacheGeneration !== this.publicCacheGeneration
        || requestGeneration !== this.listRequestGeneration
        || conversationId !== this.listConversationId) return;
      this.listCells = reconcileAccessibleListWindow(priorCells, incomingCells, conversationId);
      const frontier = accessibleListRefreshFrontier({
        reset,
        priorCells,
        incomingCells,
        priorBeforeSeq,
        priorHasMore,
        nextBeforeSeq: beforeSeq,
        nextHasMore: hasMore,
      });
      this.listBeforeSeq = frontier.beforeSeq;
      this.listHasMore = frontier.hasMore;
      this.listWindowRevision = authoritativeListRevision;
      this.root.querySelector('[data-wall-action="load-more"]').hidden = !this.listHasMore;
      this.renderList();
      this.scheduleListReconciliation(this.activationGeneration, this.identityGeneration);
      this.restoreAccessibleListView(viewState, requestGeneration, conversationId);
    } catch (error) {
      this.announce(`The wall list could not refresh: ${error.message}`);
      this.restoreAccessibleListView(viewState, requestGeneration, conversationId);
    }
  }

  renderList() {
    const title = this.root.querySelector("#honeyListTitle");
    const allButton = this.root.querySelector('[data-wall-action="list-all"]');
    const conversation = this.listConversationId ? this.conversations.get(this.listConversationId) : null;
    const participantNames = conversation?.participants?.map((participant) => participant.name).filter(Boolean) || [];
    setTextIfChanged(title, this.listConversationId
      ? (participantNames.length ? `Conversation with ${participantNames.join(", ")}` : "One Hive conversation")
      : "Every comb with its connections");
    allButton.hidden = !this.listConversationId;
    for (const node of [...this.listItems.childNodes]) {
      if (node.nodeType !== 1) node.remove();
    }
    const existing = new Map([...this.listItems.querySelectorAll("[data-wall-list-row]")]
      .map((article) => [article.dataset.wallListRow, article]));
    const desiredIds = new Set();
    let cursor = this.listItems.firstElementChild;
    for (const cell of this.listCells) {
      desiredIds.add(cell.id);
      let article = existing.get(cell.id);
      if (!article) {
        article = document.createElement("article");
        article.className = "honey-list-item";
        article.dataset.wallListRow = cell.id;
        const heading = document.createElement("h3");
        heading.dataset.wallListPart = "heading";
        const meta = document.createElement("p");
        meta.className = "honey-list-meta";
        meta.dataset.wallListPart = "meta";
        const relation = document.createElement("p");
        relation.className = "honey-list-relation";
        relation.dataset.wallListPart = "relation";
        const body = document.createElement("p");
        body.dataset.wallListPart = "body";
        const open = document.createElement("button");
        open.type = "button";
        open.className = "honey-tool";
        open.dataset.wallListPart = "open";
        open.textContent = "Open this comb";
        const openConversation = document.createElement("button");
        openConversation.type = "button";
        openConversation.className = "honey-tool";
        openConversation.dataset.wallListPart = "conversation";
        openConversation.textContent = "Open conversation";
        article.append(heading, meta, relation, body, open, openConversation);
      }
      article.style.setProperty("--thread-hue", String(hueFor(cell.conversationId)));
      article.dataset.threadPattern = String(patternFor(cell.conversationId));
      const heading = article.querySelector('[data-wall-list-part="heading"]');
      setTextIfChanged(heading, cell.author || "Open space kept");
      const meta = article.querySelector('[data-wall-list-part="meta"]');
      setTextIfChanged(meta, `${cell.membership === "alumni" ? "DHG alumni" : cell.membership === "guest" ? "Guest" : "Hive"} · ${timeLabel(cell.createdAt)} · comb ${cell.q}, ${cell.r}`);
      const body = article.querySelector('[data-wall-list-part="body"]');
      setTextIfChanged(body, statusText(cell));
      const relation = article.querySelector('[data-wall-list-part="relation"]');
      const parent = cell.replyToId ? this.cellsById.get(cell.replyToId) : null;
      const growthParent = cell.growthParentId ? this.cellsById.get(cell.growthParentId) : null;
      const conversation = this.conversations.get(cell.conversationId);
      const anchorNames = (cell.origin?.anchors || []).map((anchor) => (
        this.cellsById.get(anchor.cellId)?.author || "a nearby conversation"
      ));
      const relationText = cell.replyToId
        ? `Reply to ${parent?.author || "another comb"}`
        : cell.origin?.kind === "branch"
          ? `New branch from ${anchorNames[0] || "a nearby conversation"}`
          : cell.origin?.kind === "meet"
            ? `Meeting point for ${anchorNames.length || 2} nearby conversations${anchorNames.length ? `: ${anchorNames.join(", ")}` : ""}`
            : conversation?.rootCellId === cell.id
              ? "Conversation root"
              : `Continuation from ${growthParent?.author || "a nearby comb"}`;
      const people = this.conversations.get(cell.conversationId)?.participants
        ?.map((participant) => participant.name).filter(Boolean).slice(0, 6) || [];
      setTextIfChanged(relation, people.length ? `${relationText} · Conversation: ${people.join(", ")}` : relationText);
      const open = article.querySelector('[data-wall-list-part="open"]');
      open.dataset.wallListCell = cell.id;
      open.dataset.wallListFocus = `cell:${cell.id}`;
      open.setAttribute("aria-label", `Open comb by ${cell.author || "a Hive member"} at ${cell.q}, ${cell.r}. ${relationText}.`);
      const openConversation = article.querySelector('[data-wall-list-part="conversation"]');
      openConversation.dataset.wallListConversation = cell.conversationId;
      openConversation.dataset.wallListFocus = `conversation:${cell.id}`;
      openConversation.setAttribute("aria-label", `Open the conversation around ${cell.author || `comb ${cell.q}, ${cell.r}`}${people.length ? ` with ${people.join(", ")}` : ""}.`);
      if (article !== cursor) this.listItems.insertBefore(article, cursor);
      cursor = article.nextElementSibling;
    }
    for (const [cellId, article] of existing) {
      if (!desiredIds.has(cellId)) article.remove();
    }
    let empty = this.listItems.querySelector(".honey-list-empty");
    if (!this.listCells.length) {
      if (!empty) {
        empty = document.createElement("p");
        empty.className = "honey-list-empty";
        this.listItems.append(empty);
      }
      setTextIfChanged(empty, "The wall is ready for its first comb.");
    } else empty?.remove();
  }

  async openListConversation(conversationId) {
    if (!conversationId) return;
    this.discardAccessibleListWindow();
    this.listConversationId = conversationId;
    const identityGeneration = this.identityGeneration;
    await this.refreshList(false);
    if (identityGeneration !== this.identityGeneration || this.listConversationId !== conversationId || this.mode !== "list") return;
    this.list?.focus({ preventScroll: true });
  }

  async showAllListMessages() {
    this.discardAccessibleListWindow();
    const identityGeneration = this.identityGeneration;
    await this.refreshList(false);
    if (identityGeneration !== this.identityGeneration || this.listConversationId !== null || this.mode !== "list") return;
    this.list?.focus({ preventScroll: true });
  }

  async readAuthoritativeCell(cellId, contextCurrent) {
    for (let attempt = 0; attempt < 2; attempt += 1) {
      const result = await this.backend.wallCell(cellId);
      if (!contextCurrent()) return null;
      if (!result?.cell || !result?.bounds || !Number.isSafeInteger(Number(result.bounds.revision))) continue;
      const viewerAdvanced = this.viewerProjectionAdvanced(result.bounds);
      if (!this.applyAuthoritativeBounds(result.bounds)) continue;
      if (viewerAdvanced && this.hasPublicProjection()) {
        const rebuild = this.captureProjectionRebuild();
        this.invalidatePublicProjectionCache({ resetConversation: false });
        return { rebuildRequired: true, rebuild };
      }
      if (this.hasUnaccountedPublicRevision(Number(result.bounds.revision))) {
        const rebuild = this.captureProjectionRebuild();
        this.invalidatePublicProjectionCache({ resetConversation: false });
        return { rebuildRequired: true, rebuild };
      }
      return result;
    }
    return null;
  }

  async openListCell(cellId, opener = null, rebuildAttempt = 0) {
    if (!cellId || this.mode !== "list") return false;
    const generation = ++this.focusGeneration;
    const activationGeneration = this.activationGeneration;
    const identityGeneration = this.identityGeneration;
    const cacheGeneration = this.publicCacheGeneration;
    const conversationId = this.listConversationId;
    const contextCurrent = () => (
      generation === this.focusGeneration
      && this.isContextCurrent(activationGeneration, identityGeneration)
      && cacheGeneration === this.publicCacheGeneration
      && this.mode === "list"
      && conversationId === this.listConversationId
    );
    try {
      const result = await this.readAuthoritativeCell(cellId, contextCurrent);
      if (result?.rebuildRequired) {
        await this.rebuildPublicProjection(result.rebuild);
        if (rebuildAttempt >= 1 || this.mode !== "list" || conversationId !== this.listConversationId) return false;
        const focusKey = opener?.dataset?.wallListFocus;
        const rebuiltOpener = focusKey
          ? this.list?.querySelector(`[data-wall-list-focus="${escapeSelectorValue(focusKey)}"]`)
          : this.list;
        return this.openListCell(cellId, rebuiltOpener || this.list, rebuildAttempt + 1);
      }
      if (!result || !contextCurrent()) return false;
      if (result.conversation?.id) this.conversations.set(result.conversation.id, result.conversation);
      const cell = this.mergeCell(result.cell, { renderList: false });
      if (!cell) return false;
      this.renderList();
      this.openReader(cell, opener);
      return true;
    } catch {
      return false;
    }
  }

  async focusCell(cellId, options = {}, rebuildAttempt = 0) {
    const generation = ++this.focusGeneration;
    const activationGeneration = this.activationGeneration;
    const identityGeneration = this.identityGeneration;
    const cacheGeneration = this.publicCacheGeneration;
    if (!this.isContextCurrent(activationGeneration, identityGeneration)) return false;
    const contextCurrent = () => (
      generation === this.focusGeneration
      && this.isContextCurrent(activationGeneration, identityGeneration)
      && cacheGeneration === this.publicCacheGeneration
    );
    let result;
    try {
      result = await this.readAuthoritativeCell(cellId, contextCurrent);
    } catch {
      return false;
    }
    if (result?.rebuildRequired) {
      await this.rebuildPublicProjection(result.rebuild);
      if (rebuildAttempt >= 1 || !this.isContextCurrent(activationGeneration, identityGeneration)) return false;
      return this.focusCell(cellId, options, rebuildAttempt + 1);
    }
    if (!result || !contextCurrent()) return false;
    let cell = this.mergeCell(result.cell);
    if (!cell) return false;
    if (result.conversation?.id) this.conversations.set(result.conversation.id, result.conversation);
    if (generation !== this.focusGeneration
      || !this.isContextCurrent(activationGeneration, identityGeneration)) return false;
    this.setMode("wall");
    const modeGeneration = this.focusGeneration;
    this.revealCoordinate(cell.q, cell.r);
    const loaded = await this.loadChunk(chunkFor(cell.q, cell.r), true, Number(this.bounds.revision) || 0);
    if (modeGeneration !== this.focusGeneration
      || !this.isContextCurrent(activationGeneration, identityGeneration)) return false;
    if (!loaded?.ok) return false;
    this.wallWindowRevision = Math.max(Number(this.wallWindowRevision) || 0, Number(loaded.revision) || 0);
    this.render();
    if (options.open !== false) this.openReader(this.cellsById.get(cell.id) || cell);
    if (this.onCellLink) this.onCellLink(cell.id);
    return true;
  }

  nudgeAround(placedCell) {
    if (globalThis.matchMedia?.("(prefers-reduced-motion: reduce)").matches) return;
    for (const cell of this.cells.values()) {
      const distance = hexDistance(cell.q, cell.r, placedCell.q, placedCell.r);
      if (distance < 1 || distance > 3) continue;
      const node = this.nodes.get(cellKey(cell.q, cell.r));
      if (!node) continue;
      const amount = distance === 1 ? 9 : distance === 2 ? 6 : 3;
      const from = axialToPixel(placedCell.q, placedCell.r, 1);
      const to = axialToPixel(cell.q, cell.r, 1);
      const length = Math.hypot(to.x - from.x, to.y - from.y) || 1;
      node.style.setProperty("--nudge-x", `${((to.x - from.x) / length) * amount}px`);
      node.style.setProperty("--nudge-y", `${((to.y - from.y) / length) * amount}px`);
      node.classList.remove("is-nudged");
      void node.offsetWidth;
      node.classList.add("is-nudged");
    }
  }

  announce(message) {
    if (!this.live) return;
    this.live.textContent = "";
    if (this.announceFrame) cancelAnimationFrame(this.announceFrame);
    this.announceFrame = requestAnimationFrame(() => {
      this.announceFrame = null;
      if (this.live) this.live.textContent = message;
    });
  }
}

export function createHiveWall(options) {
  return new HiveWallController(options);
}
