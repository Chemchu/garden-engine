(function () {
  const root = document.documentElement;

  function clearStylePreview() {
    for (const property of Array.from(root.style)) {
      if (
        property.startsWith("--color-") ||
        property.startsWith("--font-") ||
        property === "--radius-box"
      ) {
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

  function fontUrl(font, weight) {
    return `url(${form.dataset.fontCdn}/${font.id}@latest/${font.subset}-${weight}-normal.woff2)`;
  }

  const MAX_LOADING = 6;
  const PAGE_SIZE = 80;
  const pickerFaces = new Map();
  const waiting = [];
  let loading = 0;

  function pickerWeight(font) {
    return font.weights.includes(400) ? 400 : closestWeight(font, 400);
  }

  function loadNext() {
    while (loading < MAX_LOADING && waiting.length > 0) {
      const { font, resolve } = waiting.shift();
      const family = `picker-${font.id}`;
      const weight = pickerWeight(font);
      const face = new FontFace(family, fontUrl(font, weight), { weight: String(weight) });
      document.fonts.add(face);
      loading += 1;
      face
        .load()
        .then(
          () => resolve(family),
          () => resolve(null),
        )
        .finally(() => {
          loading -= 1;
          loadNext();
        });
    }
  }

  function showInOwnFont(element, font) {
    if (!pickerFaces.has(font.id)) {
      pickerFaces.set(
        font.id,
        new Promise((resolve) => {
          waiting.push({ font, resolve });
          loadNext();
        }),
      );
    }
    pickerFaces.get(font.id).then((family) => {
      if (family && element.dataset.fontId === font.id) {
        element.style.fontFamily = `"${family}", sans-serif`;
        element.style.fontWeight = String(pickerWeight(font));
      }
    });
  }

  function isKeep(value) {
    const wanted = value.trim().toLowerCase();
    return wanted === "" || wanted === "keep";
  }

  const pickers = new Map();

  function previewFont(role) {
    pickers.get(role)?.sync();
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
    const face = new FontFace(family, fontUrl(font, weight), { weight });
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

  const radius = form.querySelector("[data-radius]");
  const radiusValue = document.getElementById("radius-value");

  function showRadius() {
    radiusValue.textContent = `${radius.value}px`;
  }

  function previewRadius() {
    root.style.setProperty("--radius-box", `${radius.value}px`);
    showRadius();
  }

  radius.addEventListener("input", previewRadius);

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
      showRadius();
      ["body", "heading"].forEach(previewFont);
    }
  }

  form.querySelectorAll("[data-reset-group]").forEach((button) => {
    button.addEventListener("click", () => reset(button.dataset.resetGroup));
  });

  const categoryNames = new Map();

  function setUpPicker(role) {
    const picker = form.querySelector(`[data-font-picker="${role}"]`);
    const input = field(role, "font");
    if (!picker || catalog.length === 0) {
      return;
    }

    const toggle = picker.querySelector("[data-picker-toggle]");
    const label = picker.querySelector("[data-picker-label]");
    const panel = picker.querySelector("[data-picker-panel]");
    const search = picker.querySelector("[data-picker-search]");
    const list = picker.querySelector("[role=listbox]");
    const empty = picker.querySelector("[data-picker-empty]");
    const chips = Array.from(picker.querySelectorAll("[data-picker-category]"));
    chips.forEach((chip) => categoryNames.set(chip.dataset.pickerCategory, chip.textContent));

    let category = "";
    let options = [];
    let rendered = 0;
    let active = -1;

    const observer =
      "IntersectionObserver" in window
        ? new IntersectionObserver(
            (entries) => {
              for (const entry of entries) {
                if (entry.isIntersecting) {
                  observer.unobserve(entry.target);
                  const name = entry.target.querySelector("[data-font-id]");
                  showInOwnFont(name, byId.get(name.dataset.fontId));
                }
              }
            },
            { root: list, rootMargin: "200px 0px" },
          )
        : null;

    function selectedIndex() {
      const value = input.value;
      if (isKeep(value)) {
        return 0;
      }
      const font = findFont(value);
      return font ? options.indexOf(font) : -1;
    }

    function option(font, index) {
      const item = document.createElement("li");
      item.id = `${role}-font-option-${index}`;
      item.dataset.index = String(index);
      item.setAttribute("role", "option");
      item.className =
        "flex flex-col px-2 py-1 cursor-pointer rounded-box hover:bg-shade-color aria-selected:text-primary-color";

      const name = document.createElement("span");
      name.className = "text-lg truncate";
      const meta = document.createElement("span");
      meta.className = "text-xs opacity-70";
      if (font) {
        name.textContent = font.family;
        name.dataset.fontId = font.id;
        const weights = picker.dataset.weightsLabel.replace("{}", font.weights.length);
        meta.textContent = `${categoryNames.get(font.category) || font.category} · ${weights}`;
      } else {
        name.textContent = picker.dataset.keepLabel;
      }
      item.append(name, meta);
      if (font) {
        observer?.observe(item);
      }
      return item;
    }

    function renderMore() {
      const page = options.slice(rendered, rendered + PAGE_SIZE);
      list.append(...page.map((font, offset) => option(font, rendered + offset)));
      rendered += page.length;
    }

    function renderUpTo(index) {
      while (index >= rendered && rendered < options.length) {
        renderMore();
      }
    }

    function setActive(index) {
      const items = list.children;
      items[active]?.classList.remove("bg-shade-color");
      active = Math.max(-1, Math.min(index, options.length - 1));
      renderUpTo(active);
      const item = items[active];
      if (item) {
        item.classList.add("bg-shade-color");
        search.setAttribute("aria-activedescendant", item.id);
        item.scrollIntoView?.({ block: "nearest" });
      } else {
        search.removeAttribute("aria-activedescendant");
      }
    }

    function render() {
      const query = search.value.trim().toLowerCase();
      const matches = catalog.filter(
        (font) =>
          (!category || font.category === category) &&
          (!query || font.family.toLowerCase().includes(query) || font.id.includes(query)),
      );
      if (query) {
        const startsLate = (font) => Number(!font.family.toLowerCase().startsWith(query));
        matches.sort((a, b) => startsLate(a) - startsLate(b));
      }

      observer?.disconnect();
      options = [null, ...matches];
      rendered = 0;
      active = -1;
      list.replaceChildren();
      renderMore();
      empty.hidden = matches.length > 0;

      const selected = selectedIndex();
      renderUpTo(selected);
      list.children[selected]?.setAttribute("aria-selected", "true");
      setActive(selected);
    }

    function open() {
      panel.hidden = false;
      toggle.setAttribute("aria-expanded", "true");
      search.value = "";
      render();
      search.focus();
    }

    function close() {
      panel.hidden = true;
      toggle.setAttribute("aria-expanded", "false");
    }

    function choose(index) {
      const font = options[index];
      if (index < 0 || font === undefined) {
        return;
      }
      input.value = font ? font.family : "keep";
      input.dispatchEvent(new Event("change"));
      close();
      toggle.focus();
    }

    toggle.addEventListener("click", () => (panel.hidden ? open() : close()));
    toggle.addEventListener("keydown", (event) => {
      if (event.key === "ArrowDown" && panel.hidden) {
        event.preventDefault();
        open();
      }
    });

    search.addEventListener("input", render);
    search.addEventListener("keydown", (event) => {
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        setActive(active + (event.key === "ArrowDown" ? 1 : -1));
      } else if (event.key === "Enter") {
        event.preventDefault();
        choose(active);
      } else if (event.key === "Escape") {
        event.preventDefault();
        close();
        toggle.focus();
      }
    });

    chips.forEach((chip) =>
      chip.addEventListener("click", () => {
        category = chip.dataset.pickerCategory;
        chips.forEach((other) => other.setAttribute("aria-pressed", String(other === chip)));
        render();
        search.focus();
      }),
    );

    list.addEventListener("scroll", () => {
      if (list.scrollTop + list.clientHeight >= list.scrollHeight - 200) {
        renderMore();
      }
    });
    list.addEventListener("mousedown", (event) => event.preventDefault());
    list.addEventListener("click", (event) => {
      const item = event.target.closest("[role=option]");
      if (item) {
        choose(Number(item.dataset.index));
      }
    });

    picker.addEventListener("focusout", (event) => {
      if (!picker.contains(event.relatedTarget)) {
        close();
      }
    });

    function sync() {
      const value = input.value;
      const font = findFont(value);
      label.style.fontFamily = "";
      label.style.fontWeight = "";
      delete label.dataset.fontId;
      if (font) {
        label.textContent = font.family;
        label.dataset.fontId = font.id;
        showInOwnFont(label, font);
      } else {
        label.textContent = isKeep(value) ? picker.dataset.keepLabel : value;
      }
    }

    pickers.set(role, { sync });
    picker.hidden = false;
    input.hidden = true;
    form.querySelector("[data-font-hint]")?.setAttribute("hidden", "");
  }

  ["body", "heading"].forEach(setUpPicker);
  previewRadius();
  ["body", "heading"].forEach(previewFont);
})();
