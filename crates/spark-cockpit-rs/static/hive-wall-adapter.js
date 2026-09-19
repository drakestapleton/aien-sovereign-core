/**
 * Sovereign Hive Honeycomb Wall Adapter
 * Integrates Drake's Hexagonal Honeycomb Message Wall into the Native Agent Hive Cockpit.
 */

(function () {
  const DEFAULT_RADIUS = 88;
  let wallController = null;

  function formatCombForWall(comb) {
    if (!comb) return null;
    return {
      id: String(comb.id),
      q: Number(comb.q),
      r: Number(comb.r),
      author: comb.author || "AIEN",
      role: comb.role || "agent",
      body: comb.content || comb.body || "",
      content: comb.content || comb.body || "",
      status: comb.status || "visible",
      membership: comb.role === "operator" ? "guest" : "alumni",
      conversationId: comb.parent_id || comb.id,
      growthParentId: comb.parent_id || null,
      createdAt: typeof comb.created_at === "string" ? new Date(comb.created_at).getTime() : (comb.createdAt || Date.now()),
      created_at: comb.created_at || new Date().toISOString(),
      intent: comb.intent || "independent",
      origin: comb.intent ? {
        kind: comb.intent,
        anchors: comb.parent_id ? [{ cellId: comb.parent_id }] : []
      } : null,
      isMine: comb.role === "operator" || comb.author === "Drake" || comb.author === "AIEN",
      neighbors: Array.isArray(comb.neighbors) ? comb.neighbors : []
    };
  }

  const cockpitHiveBackend = {
    async wallBounds() {
      try {
        const res = await fetch("/api/hive/cells");
        if (!res.ok) throw new Error("HTTP " + res.status);
        const data = await res.json();
        return data.bounds || {
          revision: data.combs ? data.combs.length : 1,
          viewerRevision: data.combs ? data.combs.length : 1,
          radius: 5,
          messageCount: data.combs ? data.combs.length : 1,
          readOnly: false,
          updatedAt: Date.now()
        };
      } catch (err) {
        console.warn("[Hive Wall] wallBounds fallback:", err);
        return {
          revision: 1,
          viewerRevision: 1,
          radius: 5,
          messageCount: 1,
          readOnly: false,
          updatedAt: Date.now()
        };
      }
    },

    async wallRegion(region) {
      try {
        const res = await fetch("/api/hive/cells");
        if (!res.ok) throw new Error("HTTP " + res.status);
        const data = await res.json();
        const rawCombs = data.combs || data.cells || [];
        const filtered = rawCombs.filter(c => 
          c.q >= region.qMin && c.q <= region.qMax &&
          c.r >= region.rMin && c.r <= region.rMax
        );
        const cells = filtered.map(formatCombForWall);
        return {
          bounds: data.bounds,
          region: {
            qMin: region.qMin,
            qMax: region.qMax,
            rMin: region.rMin,
            rMax: region.rMax
          },
          cells,
          conversations: [],
          truncated: false
        };
      } catch (err) {
        console.error("[Hive Wall] wallRegion error:", err);
        return {
          bounds: { revision: 1, radius: 5, messageCount: 0, readOnly: false, updatedAt: Date.now() },
          region,
          cells: [],
          conversations: [],
          truncated: false
        };
      }
    },

    async wallPlaceCell(entry) {
      const payload = {
        q: entry.q,
        r: entry.r,
        author: "Drake",
        role: "operator",
        content: entry.body,
        intent: entry.intent || "independent",
        parent_id: entry.replyToId || (entry.anchorCellIds && entry.anchorCellIds[0]) || null
      };

      const res = await fetch("/api/hive/comb", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload)
      });

      if (!res.ok) {
        const err = await res.json().catch(() => ({ error: "Failed to place comb" }));
        throw new Error(err.error || "Placement rejected");
      }

      const data = await res.json();
      return {
        bounds: data.bounds,
        cell: formatCombForWall(data.comb || data.cell)
      };
    },

    async wallCell(cellId) {
      const res = await fetch("/api/hive/cells");
      const data = await res.json();
      const rawCombs = data.combs || data.cells || [];
      const found = rawCombs.find(c => String(c.id) === String(cellId));
      return {
        bounds: data.bounds,
        cell: found ? formatCombForWall(found) : null
      };
    },

    subscribeWallChanges(callback) {
      let lastRevision = 0;
      const timer = setInterval(async () => {
        try {
          const res = await fetch("/api/hive/bounds");
          if (!res.ok) return;
          const data = await res.json();
          const rev = data.bounds ? data.bounds.revision : (data.count || 0);
          if (rev !== lastRevision) {
            lastRevision = rev;
            callback({
              type: "bounds",
              revision: rev,
              bounds: data.bounds
            });
            updateWallHeaderBadge(data.bounds);
          }
        } catch (e) {}
      }, 4000);

      return () => clearInterval(timer);
    },

    async wallIdentityKey() {
      return "hive:operator";
    }
  };

  function updateWallHeaderBadge(bounds) {
    if (!bounds) return;
    const countEl = document.getElementById("hive-wall-count");
    const radiusEl = document.getElementById("hive-wall-radius");
    if (countEl) countEl.textContent = bounds.count ?? bounds.messageCount ?? 0;
    if (radiusEl) radiusEl.textContent = bounds.radius ?? 0;
  }

  function initHiveWall() {
    const mount = document.getElementById("hive-wall-container");
    if (!mount) return;

    if (!wallController && window.HiveWall && typeof window.HiveWall.createHiveWall === "function") {
      try {
        wallController = window.HiveWall.createHiveWall({
          root: mount,
          backend: cockpitHiveBackend,
          getUser: () => ({ tag: "Drake", membership: "alumni" }),
          toast: (msg) => {
            console.log("[Hive Wall Toast]", msg);
          },
          hexRadius: DEFAULT_RADIUS,
        });
        wallController.activate();
        console.log("✓ Hexagonal Honeycomb Message Wall active in Hive Pulse");
      } catch (err) {
        console.error("Failed to initialize HiveWallController:", err);
      }
    } else if (wallController) {
      // Refresh visible region on re-tab
      if (typeof wallController.refreshVisible === "function") {
        wallController.refreshVisible(true);
      }
    }

    // Update telemetry badge
    fetch("/api/hive/bounds")
      .then(r => r.json())
      .then(d => {
        if (d.bounds) updateWallHeaderBadge(d.bounds);
      })
      .catch(() => {});
  }

  async function emitSocraticInquiryModal() {
    const question = prompt("Enter Socratic inquiry to emit onto the hexagonal honeycomb lattice:");
    if (!question || !question.trim()) return;

    try {
      const res = await fetch("/api/hive/comb", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          author: "AIEN (Socratic)",
          role: "socratic",
          content: question.trim(),
          intent: "branch"
        })
      });

      if (!res.ok) {
        const err = await res.json().catch(() => ({ error: "Emission failed" }));
        alert("Failed to emit comb: " + (err.error || "Unknown error"));
        return;
      }

      const data = await res.json();
      console.log("✓ Socratic Comb placed:", data);
      if (wallController && typeof wallController.refreshVisible === "function") {
        wallController.refreshVisible(true);
      }
    } catch (e) {
      alert("Error emitting Socratic inquiry: " + e.message);
    }
  }

  // Expose globally
  window.initHiveWall = initHiveWall;
  window.emitSocraticInquiryModal = emitSocraticInquiryModal;

  // Wire up manual refresh and socratic buttons if present
  document.addEventListener("DOMContentLoaded", () => {
    const refreshBtn = document.getElementById("btn-hive-wall-refresh");
    if (refreshBtn) {
      refreshBtn.addEventListener("click", () => {
        if (wallController && typeof wallController.refreshVisible === "function") {
          wallController.refreshVisible(true);
        }
      });
    }

    const socraticBtn = document.getElementById("btn-hive-wall-socratic");
    if (socraticBtn) {
      socraticBtn.addEventListener("click", emitSocraticInquiryModal);
    }
  });
})();
