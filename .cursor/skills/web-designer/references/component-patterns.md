# Component Patterns Reference

Reusable UI patterns for the website builder skill. Copy-adapt as needed.

---

## Navigation

### Sticky Navbar with scroll-aware background
```jsx
const [scrolled, setScrolled] = useState(false);
useEffect(() => {
  const onScroll = () => setScrolled(window.scrollY > 50);
  window.addEventListener('scroll', onScroll);
  return () => window.removeEventListener('scroll', onScroll);
}, []);

<nav style={{
  position: 'fixed', top: 0, width: '100%', zIndex: 100,
  background: scrolled ? 'rgba(10,10,10,0.95)' : 'transparent',
  backdropFilter: scrolled ? 'blur(12px)' : 'none',
  transition: 'all 0.3s ease',
  padding: '1rem 2rem',
  display: 'flex', justifyContent: 'space-between', alignItems: 'center',
}}>
```

### Mobile hamburger menu
```jsx
const [menuOpen, setMenuOpen] = useState(false);
// Toggle a full-screen overlay or slide-in sidebar
```

---

## Hero Variants

### Full-viewport with diagonal split
```jsx
<section style={{
  minHeight: '100vh',
  display: 'grid',
  gridTemplateColumns: '1fr 1fr',
  clipPath: 'polygon(0 0, 100% 0, 100% 85%, 0 100%)',
}}>
  <div>{/* Text */}</div>
  <div style={{ background: 'var(--accent)' }}>{/* Visual */}</div>
</section>
```

### Centered with oversized typography
```jsx
<section style={{
  minHeight: '100vh', display: 'flex',
  flexDirection: 'column', alignItems: 'center', justifyContent: 'center',
  textAlign: 'center', padding: '2rem',
}}>
  <h1 style={{ fontSize: 'clamp(3rem, 10vw, 9rem)', lineHeight: 1, fontWeight: 900 }}>
    {headline}
  </h1>
</section>
```

### Asymmetric with floating card
```jsx
<section style={{ display: 'grid', gridTemplateColumns: '3fr 2fr', gap: '4rem', minHeight: '100vh', alignItems: 'center' }}>
  <div>{/* Left: big text */}</div>
  <div style={{ transform: 'translateY(-2rem)', boxShadow: '0 40px 80px rgba(0,0,0,0.3)' }}>
    {/* Right: floating card/mockup */}
  </div>
</section>
```

---

## Cards

### Hover-lift card
```jsx
const cardStyle = {
  background: 'var(--card-bg)',
  borderRadius: '1rem',
  padding: '2rem',
  transition: 'transform 0.25s ease, box-shadow 0.25s ease',
  cursor: 'pointer',
};
const cardHoverStyle = {
  transform: 'translateY(-8px)',
  boxShadow: '0 20px 40px rgba(0,0,0,0.15)',
};
// Use onMouseEnter/Leave to toggle hover state
```

### Glass card
```jsx
{
  background: 'rgba(255,255,255,0.08)',
  backdropFilter: 'blur(20px)',
  border: '1px solid rgba(255,255,255,0.15)',
  borderRadius: '1.5rem',
  padding: '2rem',
}
```

### Border-accent card (left stripe)
```jsx
{
  borderLeft: '4px solid var(--accent)',
  paddingLeft: '1.5rem',
  background: 'var(--surface)',
}
```

---

## Buttons

### Primary CTA
```jsx
<button style={{
  background: 'var(--accent)',
  color: 'white',
  border: 'none',
  padding: '1rem 2.5rem',
  borderRadius: '0.5rem',
  fontSize: '1.1rem',
  fontWeight: 700,
  cursor: 'pointer',
  transition: 'transform 0.15s, box-shadow 0.15s',
  letterSpacing: '0.02em',
}} onMouseEnter={e => { e.target.style.transform='scale(1.04)'; e.target.style.boxShadow='0 8px 24px rgba(0,0,0,0.2)' }}
   onMouseLeave={e => { e.target.style.transform='scale(1)'; e.target.style.boxShadow='none' }}>
  Get Started
</button>
```

### Ghost / outline button
```jsx
<button style={{
  background: 'transparent',
  color: 'var(--accent)',
  border: '2px solid var(--accent)',
  padding: '0.9rem 2.2rem',
  borderRadius: '0.5rem',
  cursor: 'pointer',
  transition: 'background 0.2s, color 0.2s',
}}>
```

---

## Forms

### Minimal contact form
```jsx
const [form, setForm] = useState({ name: '', email: '', message: '' });
const [sent, setSent] = useState(false);

const handleSubmit = (e) => {
  e.preventDefault();
  // In a real app, POST to API
  setSent(true);
};

<form onSubmit={handleSubmit} style={{ display: 'flex', flexDirection: 'column', gap: '1rem', maxWidth: '480px' }}>
  {['name','email'].map(field => (
    <input key={field}
      placeholder={field.charAt(0).toUpperCase() + field.slice(1)}
      value={form[field]}
      onChange={e => setForm({...form, [field]: e.target.value})}
      style={{ padding: '0.875rem 1rem', borderRadius: '0.5rem', border: '1px solid var(--border)', background: 'var(--surface)', fontSize: '1rem' }}
    />
  ))}
  <textarea rows={4} placeholder="Message"
    value={form.message}
    onChange={e => setForm({...form, message: e.target.value})}
    style={{ padding: '0.875rem 1rem', borderRadius: '0.5rem', border: '1px solid var(--border)', background: 'var(--surface)', fontSize: '1rem', resize: 'vertical' }}
  />
  <button type="submit">{sent ? '✓ Sent!' : 'Send Message'}</button>
</form>
```

---

## Scroll-triggered Reveal

```jsx
function useReveal() {
  const ref = useRef(null);
  const [visible, setVisible] = useState(false);
  useEffect(() => {
    const observer = new IntersectionObserver(
      ([entry]) => { if (entry.isIntersecting) setVisible(true); },
      { threshold: 0.15 }
    );
    if (ref.current) observer.observe(ref.current);
    return () => observer.disconnect();
  }, []);
  return [ref, visible];
}

// Usage:
const [ref, visible] = useReveal();
<div ref={ref} style={{
  opacity: visible ? 1 : 0,
  transform: visible ? 'translateY(0)' : 'translateY(30px)',
  transition: 'opacity 0.6s ease, transform 0.6s ease',
}}>
```

---

## Stats / Numbers Section

```jsx
const stats = [
  { value: '10K+', label: 'Happy Customers' },
  { value: '99%', label: 'Uptime' },
  { value: '4.9★', label: 'Average Rating' },
];
<div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: '2rem', textAlign: 'center' }}>
  {stats.map(s => (
    <div key={s.label}>
      <div style={{ fontSize: '3rem', fontWeight: 900, color: 'var(--accent)' }}>{s.value}</div>
      <div style={{ color: 'var(--muted)' }}>{s.label}</div>
    </div>
  ))}
</div>
```

---

## Footer

```jsx
<footer style={{
  borderTop: '1px solid var(--border)',
  padding: '3rem 2rem',
  display: 'flex',
  justifyContent: 'space-between',
  alignItems: 'center',
  flexWrap: 'wrap',
  gap: '1rem',
}}>
  <span style={{ fontWeight: 700, fontSize: '1.2rem' }}>Brand</span>
  <nav style={{ display: 'flex', gap: '2rem' }}>
    {['About', 'Work', 'Contact'].map(l => <a key={l} href="#" style={{ color: 'var(--muted)', textDecoration: 'none' }}>{l}</a>)}
  </nav>
  <span style={{ color: 'var(--muted)', fontSize: '0.875rem' }}>© 2025</span>
</footer>
```