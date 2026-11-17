function themeColor(name) {
  return getComputedStyle(document.documentElement)
    .getPropertyValue(`--color-${name}-color`)
    .trim();
}
