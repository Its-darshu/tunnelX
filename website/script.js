// tunnelX landing — small interactions

// Copy install command
(function () {
  const btn = document.getElementById("copyBtn");
  const cmd = document.getElementById("installCmd");
  if (!btn || !cmd) return;

  btn.addEventListener("click", async function () {
    const text = cmd.textContent.trim();
    try {
      await navigator.clipboard.writeText(text);
    } catch (err) {
      // Fallback for non-secure contexts
      const range = document.createRange();
      range.selectNodeContents(cmd);
      const sel = window.getSelection();
      sel.removeAllRanges();
      sel.addRange(range);
      document.execCommand("copy");
      sel.removeAllRanges();
    }
    btn.classList.add("is-copied");
    setTimeout(() => btn.classList.remove("is-copied"), 1400);
  });
})();

// Copy helper for arbitrary text
async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
  } catch (err) {
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    document.body.removeChild(ta);
  }
}

// Docs code-block copy buttons
(function () {
  document.querySelectorAll(".cb").forEach((block) => {
    const btn = block.querySelector(".cb__copy");
    const code = block.querySelector("pre code, pre");
    if (!btn || !code) return;
    btn.addEventListener("click", async () => {
      await copyText(code.textContent.replace(/\n$/, ""));
      const original = btn.textContent;
      btn.textContent = "Copied";
      btn.classList.add("is-copied");
      setTimeout(() => {
        btn.textContent = original;
        btn.classList.remove("is-copied");
      }, 1400);
    });
  });
})();

// Docs sidebar toggle (mobile)
(function () {
  const toggle = document.querySelector(".docs__navtoggle");
  const nav = document.getElementById("docsNav");
  if (!toggle || !nav) return;
  toggle.addEventListener("click", () => {
    const open = nav.classList.toggle("is-open");
    toggle.setAttribute("aria-expanded", String(open));
  });
  nav.querySelectorAll("a").forEach((link) => {
    link.addEventListener("click", () => {
      if (window.matchMedia("(max-width: 900px)").matches) {
        nav.classList.remove("is-open");
        toggle.setAttribute("aria-expanded", "false");
      }
    });
  });
})();

// Docs scrollspy — highlight the section currently in view
(function () {
  const links = Array.from(document.querySelectorAll(".docs__nav a"));
  if (!links.length) return;
  const sections = links
    .map((a) => document.querySelector(a.getAttribute("href")))
    .filter(Boolean);

  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          const id = entry.target.id;
          links.forEach((a) =>
            a.classList.toggle("is-active", a.getAttribute("href") === "#" + id)
          );
        }
      });
    },
    { rootMargin: "-96px 0px -70% 0px", threshold: 0 }
  );
  sections.forEach((s) => observer.observe(s));
})();

// Mobile menu toggle
(function () {
  const toggle = document.querySelector(".nav__toggle");
  const menu = document.getElementById("mobileMenu");
  if (!toggle || !menu) return;

  toggle.addEventListener("click", function () {
    const open = menu.classList.toggle("is-open");
    toggle.setAttribute("aria-expanded", String(open));
  });

  menu.querySelectorAll("a").forEach((link) => {
    link.addEventListener("click", () => {
      menu.classList.remove("is-open");
      toggle.setAttribute("aria-expanded", "false");
    });
  });
})();
