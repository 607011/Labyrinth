/*
   ProgressBar custom web element. 
   Copyright (c) 2020-2022 Oliver Lau <oliver@ersatzworld.net>
 */
class ProgressBar extends HTMLElement {
  constructor() {
    super()
    const shadowRoot = this.attachShadow({ mode: 'open' })
    shadowRoot.innerHTML = `
<style type="text/css">
.progress-bar {
  --progressbar-color1: transparent;
  --progressbar-color2: rgba(43, 72, 97, 0.5);
  --height: 28px;
  --size: 112px;
  display: block;
  position: fixed;
  left: 0;
  right: 0;
  top: 0;
  height: var(--height);
  background-color: var(--progressbar-color1);
  background-size: var(--size) var(--height);
}
.progress-bar:before{
  content: "";
  display: block;
  height: 100%;
  position: absolute;
  background-color: var(--progressbar-color2);
  animation-name: motion;
  animation-duration: 1000ms;
  animation-timing-function: linear;
  animation-iteration-count: infinite;
  animation-fill-mode: both;
}
@keyframes motion {
  0% { left: 0; width: 0; }
  20% { left: 20%; width: 60%; }
  40% { left: 30%; width: 20% ;}
  60% { left: 60%; width: 10%; }
  80% { left: 70%; width: 30%; }
  100% { left: 100%; width: 10%; }
}
</style>
<div class="progress-bar"></div>`;
    this.bar = shadowRoot.querySelector('.progress-bar')
    if (this.hasAttribute('height')) {
      this.bar.style.setProperty('--height', this.getAttribute('height'));
    }
    if (this.getAttribute('disabled') === 'true') {
      this.bar.style.display = 'none';
    }
  }
  static get observedAttributes() {
    return ['disabled'];
  }
  update(o) {
    this.bar.style.width = `${100 * o.value / (o.max - o.min)}%`;
  }
  attributeChangedCallback(attrName, _oldVal, newVal) {
    switch (attrName) {
      case 'disabled':
        this.bar.style.display = newVal !== 'false' ? 'none' : 'block';
        break
      default:
        break;
    }
  }
}
