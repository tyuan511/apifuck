// Apply the saved preference before first paint; storage may be unavailable.
try {
  const theme = localStorage.getItem("apifuck-theme");
  if (theme === "light" || theme === "dark")
    document.documentElement.dataset.theme = theme;
} catch {}
