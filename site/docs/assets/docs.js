/* BATHOS docs — self-contained behavior (no external references).
   1) mobile sidebar drawer  2) scroll-spy for the on-this-page TOC. */
(function () {
  // ---- mobile sidebar drawer ----
  var body = document.body;
  var btn = document.querySelector('.menu-btn');
  var scrim = document.querySelector('.scrim');
  function close() { body.classList.remove('nav-open'); if (btn) btn.setAttribute('aria-expanded', 'false'); }
  if (btn) {
    btn.addEventListener('click', function () {
      var open = body.classList.toggle('nav-open');
      btn.setAttribute('aria-expanded', open ? 'true' : 'false');
    });
  }
  if (scrim) scrim.addEventListener('click', close);
  document.addEventListener('keydown', function (e) { if (e.key === 'Escape') close(); });
  // close the drawer after tapping a sidebar link
  document.querySelectorAll('.sidebar a').forEach(function (a) {
    a.addEventListener('click', function () { if (window.matchMedia('(max-width:820px)').matches) close(); });
  });

  // ---- scroll-spy: highlight the current section in the TOC ----
  var tocLinks = Array.prototype.slice.call(document.querySelectorAll('.toc a[href^="#"]'));
  if (!tocLinks.length || !('IntersectionObserver' in window)) return;
  var map = {};
  var targets = [];
  tocLinks.forEach(function (a) {
    var id = decodeURIComponent(a.getAttribute('href').slice(1));
    var el = document.getElementById(id);
    if (el) { map[id] = a; targets.push(el); }
  });
  function setCurrent(id) {
    tocLinks.forEach(function (a) {
      a.setAttribute('aria-current', decodeURIComponent(a.getAttribute('href').slice(1)) === id ? 'true' : 'false');
    });
  }
  var visible = new Set();
  var obs = new IntersectionObserver(function (entries) {
    entries.forEach(function (e) {
      if (e.isIntersecting) visible.add(e.target); else visible.delete(e.target);
    });
    // pick the visible heading nearest the top
    var best = null, bestTop = Infinity;
    visible.forEach(function (el) {
      var t = el.getBoundingClientRect().top;
      if (t < bestTop) { bestTop = t; best = el; }
    });
    if (best) setCurrent(best.id);
  }, { rootMargin: '-72px 0px -68% 0px', threshold: 0 });
  targets.forEach(function (t) { obs.observe(t); });
  if (targets.length) setCurrent(targets[0].id);
})();
