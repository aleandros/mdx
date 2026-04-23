// copy-to-clipboard for install command
document.querySelectorAll('.copy-btn').forEach((btn) => {
  btn.addEventListener('click', async () => {
    const target = document.querySelector(btn.dataset.clip);
    if (!target) return;
    const text = target.textContent.trim();
    try {
      await navigator.clipboard.writeText(text);
      btn.classList.add('copied');
      const original = btn.innerHTML;
      btn.innerHTML = '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="20 6 9 17 4 12"/></svg>';
      setTimeout(() => {
        btn.classList.remove('copied');
        btn.innerHTML = original;
      }, 1600);
    } catch (err) {
      console.error('clipboard write failed', err);
    }
  });
});

// fade-in on scroll for sections
const observer = new IntersectionObserver(
  (entries) => {
    entries.forEach((e) => {
      if (e.isIntersecting) {
        e.target.style.opacity = '1';
        e.target.style.transform = 'translateY(0)';
      }
    });
  },
  { threshold: 0.08, rootMargin: '0px 0px -50px 0px' }
);

document.querySelectorAll('section').forEach((s) => {
  s.style.opacity = '0';
  s.style.transform = 'translateY(20px)';
  s.style.transition = 'opacity 0.6s ease-out, transform 0.6s ease-out';
  observer.observe(s);
});

// hero visible immediately — undo the observer setup for it
const hero = document.querySelector('.hero');
if (hero) {
  hero.style.opacity = '1';
  hero.style.transform = 'translateY(0)';
}
