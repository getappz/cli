# Animation Recipes for React

Copy-paste animation patterns. Inject keyframes via a `<style>` tag using `useEffect`.

---

## Setup: Inject Keyframes Once

```jsx
useEffect(() => {
  const style = document.createElement('style');
  style.textContent = `
    @keyframes fadeIn {
      from { opacity: 0; transform: translateY(20px); }
      to   { opacity: 1; transform: translateY(0); }
    }
    @keyframes fadeInLeft {
      from { opacity: 0; transform: translateX(-30px); }
      to   { opacity: 1; transform: translateX(0); }
    }
    @keyframes fadeInRight {
      from { opacity: 0; transform: translateX(30px); }
      to   { opacity: 1; transform: translateX(0); }
    }
    @keyframes scaleIn {
      from { opacity: 0; transform: scale(0.92); }
      to   { opacity: 1; transform: scale(1); }
    }
    @keyframes slideDown {
      from { opacity: 0; transform: translateY(-20px); }
      to   { opacity: 1; transform: translateY(0); }
    }
    @keyframes pulse {
      0%, 100% { transform: scale(1); }
      50%       { transform: scale(1.04); }
    }
    @keyframes shimmer {
      0%   { background-position: -200% center; }
      100% { background-position: 200% center; }
    }
    @keyframes spin {
      to { transform: rotate(360deg); }
    }
    @keyframes draw {
      to { stroke-dashoffset: 0; }
    }
  `;
  document.head.appendChild(style);
  return () => document.head.removeChild(style);
}, []);
```

---

## Staggered Hero Entrance

Apply progressive `animation-delay` to reveal elements in sequence:

```jsx
const heroItems = [
  { content: <span>Tag line</span>, delay: '0s', anim: 'slideDown 0.5s ease forwards' },
  { content: <h1>Big Headline</h1>, delay: '0.15s', anim: 'fadeIn 0.7s ease forwards' },
  { content: <p>Subtext</p>, delay: '0.3s', anim: 'fadeIn 0.7s ease forwards' },
  { content: <button>CTA</button>, delay: '0.5s', anim: 'scaleIn 0.5s ease forwards' },
];

{heroItems.map(({ content, delay, anim }, i) => (
  <div key={i} style={{ opacity: 0, animation: anim, animationDelay: delay }}>
    {content}
  </div>
))}
```

---

## Staggered Grid Reveal (on mount)

```jsx
{items.map((item, i) => (
  <div key={i} style={{
    opacity: 0,
    animation: 'fadeIn 0.6s ease forwards',
    animationDelay: `${i * 0.1}s`,
  }}>
    {/* card content */}
  </div>
))}
```

---

## Scroll-triggered Stagger (multiple children)

```jsx
function useStaggerReveal(count) {
  const refs = useRef([]);
  const [visible, setVisible] = useState(new Array(count).fill(false));
  useEffect(() => {
    const observers = refs.current.map((el, i) => {
      if (!el) return null;
      const obs = new IntersectionObserver(([entry]) => {
        if (entry.isIntersecting) {
          setVisible(prev => { const n = [...prev]; n[i] = true; return n; });
          obs.disconnect();
        }
      }, { threshold: 0.1 });
      obs.observe(el);
      return obs;
    });
    return () => observers.forEach(o => o?.disconnect());
  }, []);
  return [refs, visible];
}

// Usage:
const [refs, visible] = useStaggerReveal(items.length);
{items.map((item, i) => (
  <div key={i} ref={el => refs.current[i] = el} style={{
    opacity: visible[i] ? 1 : 0,
    transform: visible[i] ? 'translateY(0)' : 'translateY(40px)',
    transition: `opacity 0.6s ease ${i * 0.1}s, transform 0.6s ease ${i * 0.1}s`,
  }}>
```

---

## Shimmer Loading Skeleton

```jsx
<div style={{
  background: 'linear-gradient(90deg, var(--surface) 25%, var(--border) 50%, var(--surface) 75%)',
  backgroundSize: '200% 100%',
  animation: 'shimmer 1.5s infinite',
  borderRadius: '0.5rem',
  height: '1.2rem',
  width: '80%',
}} />
```

---

## Floating / Bobbing Element

```jsx
<div style={{
  animation: 'float 4s ease-in-out infinite',
}}>
  {/* logo, illustration, etc. */}
</div>
// Add to keyframes: @keyframes float { 0%,100%{transform:translateY(0)} 50%{transform:translateY(-16px)} }
```

---

## Text Typewriter Effect

```jsx
function Typewriter({ text, speed = 50 }) {
  const [displayed, setDisplayed] = useState('');
  useEffect(() => {
    let i = 0;
    const interval = setInterval(() => {
      setDisplayed(text.slice(0, ++i));
      if (i >= text.length) clearInterval(interval);
    }, speed);
    return () => clearInterval(interval);
  }, [text, speed]);
  return <span>{displayed}<span style={{ animation: 'pulse 1s infinite', opacity: 0.7 }}>|</span></span>;
}
```

---

## Smooth Counter (number count-up)

```jsx
function CountUp({ end, duration = 2000 }) {
  const [count, setCount] = useState(0);
  const [ref, visible] = useReveal(); // from component-patterns.md
  useEffect(() => {
    if (!visible) return;
    let start = 0;
    const step = end / (duration / 16);
    const timer = setInterval(() => {
      start += step;
      if (start >= end) { setCount(end); clearInterval(timer); }
      else setCount(Math.floor(start));
    }, 16);
    return () => clearInterval(timer);
  }, [visible, end, duration]);
  return <span ref={ref}>{count.toLocaleString()}</span>;
}
```

---

## Parallax on Scroll

```jsx
const [offset, setOffset] = useState(0);
useEffect(() => {
  const onScroll = () => setOffset(window.scrollY);
  window.addEventListener('scroll', onScroll, { passive: true });
  return () => window.removeEventListener('scroll', onScroll);
}, []);

<div style={{ transform: `translateY(${offset * 0.3}px)` }}>
  {/* background layer — moves slower than scroll */}
</div>
```

---

## Page Transition (fade in on mount)

```jsx
const [mounted, setMounted] = useState(false);
useEffect(() => { setMounted(true); }, []);

<div style={{
  opacity: mounted ? 1 : 0,
  transition: 'opacity 0.4s ease',
}}>
  {/* page content */}
</div>
```