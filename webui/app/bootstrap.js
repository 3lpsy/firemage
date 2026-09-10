import init from '/firemage.js';
init().catch(() => {
  const message = document.createElement('p');
  message.textContent = 'Firemage could not load. Reload this page to try again.';
  message.className = 'loading-screen';
  document.getElementById('main').replaceChildren(message);
});
