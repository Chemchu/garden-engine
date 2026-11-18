(function () {
  const root = document.documentElement;

  function clearStylePreview() {
    for (const property of Array.from(root.style)) {
      if (property.startsWith("--color-") || property.startsWith("--font-")) {
        root.style.removeProperty(property);
      }
    }
  }

  function clearPreview() {
    if (!document.getElementById("settings-form")) {
      clearStylePreview();
    }
  }

  if (!window.settingsPreviewCleanup) {
    window.settingsPreviewCleanup = true;
    document.addEventListener("htmx:afterSwap", clearPreview);
    document.addEventListener("htmx:historyRestore", clearPreview);
  }

  const form = document.getElementById("settings-form");
  const data = document.getElementById("font-catalog-data");
  if (!form || !data) {
    return;
  }

  const catalog = JSON.parse(data.textContent || "[]");
  const byId = new Map(catalog.map((font) => [font.id, font]));
  const byFamily = new Map(catalog.map((font) => [font.family.toLowerCase(), font]));

  function findFont(value) {
    const wanted = value.trim();
    return byId.get(wanted) || byFamily.get(wanted.toLowerCase());
  }

  function field(role, name) {
    return form.querySelector(`[name="${role}_${name}"]`);
  }

  function closestWeight(font, wanted) {
    return font.weights.reduce((best, weight) =>
      Math.abs(weight - wanted) < Math.abs(best - wanted) ? weight : best,
    );
  }

  function previewFont(role) {
    const font = findFont(field(role, "font").value);
    const weightSelect = field(role, "weight");

    weightSelect.disabled = !font;
    for (const option of weightSelect.options) {
      option.disabled = Boolean(font) && !font.weights.includes(Number(option.value));
    }

    if (!font) {
      root.style.removeProperty(`--font-${role}`);
      root.style.removeProperty(`--font-${role}-weight`);
      return;
    }

    if (weightSelect.selectedOptions[0]?.disabled) {
      weightSelect.value = String(closestWeight(font, Number(weightSelect.value)));
    }

    const weight = weightSelect.value;
    const fallback = field(role, "fallback").value;
    const family = `preview-${role}-${font.id}-${weight}`;
    const face = new FontFace(
      family,
      `url(${form.dataset.fontCdn}/${font.id}@latest/latin-${weight}-normal.woff2)`,
      { weight },
    );
    document.fonts.add(face);
    face
      .load()
      .then(() => {
        if (!form.isConnected) {
          return;
        }
        root.style.setProperty(`--font-${role}`, `"${family}", ${fallback}`);
        root.style.setProperty(`--font-${role}-weight`, weight);
      })
      .catch(() => {});
  }

  form.querySelectorAll("input[type=color]").forEach((input) => {
    input.addEventListener("input", () => {
      root.style.setProperty(`--color-${input.dataset.color}-color`, input.value);
    });
  });

  form.querySelectorAll("[data-font-role]").forEach((input) => {
    input.addEventListener("change", () => previewFont(input.dataset.fontRole));
  });

  function reset(group) {
    form.querySelectorAll(`[data-reset="${group}"]`).forEach((input) => {
      if (input.type === "checkbox") {
        input.checked = input.dataset.deployed === "true";
      } else {
        input.value = input.dataset.deployed;
      }
    });
    if (group === "styles") {
      clearStylePreview();
      ["body", "heading"].forEach(previewFont);
    }
  }

  form.querySelectorAll("[data-reset-group]").forEach((button) => {
    button.addEventListener("click", () => reset(button.dataset.resetGroup));
  });

  ["body", "heading"].forEach(previewFont);
})();
