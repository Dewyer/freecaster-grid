let key = localStorage.getItem('grid_key');
if (!key) {
    key = prompt('Enter grid key:');
    if (key) localStorage.setItem('grid_key', key);
}

document.getElementById('change-key').onclick = function() {
    localStorage.removeItem('grid_key');
    location.reload();
};

function fetchGrid() {
    fetch(`/grid/${key}`)
        .then(r => r.json())
        .then(data => {
            document.getElementById('status').textContent = `Alive: ${data.alive_nodes}, Dead: ${data.dead_nodes}, Dying: ${data.dying_nodes}, Total: ${data.total_nodes}`;
            const tbody = document.querySelector('#nodes tbody');
            tbody.innerHTML = '';
            data.nodes.forEach(node => {
                const tr = document.createElement('tr');
                tr.innerHTML = `<td>${node.name}</td><td>${node.last_poll ? node.last_poll : ''}</td><td>${node.status}</td>`;
                tr.className = node.status;
                tbody.appendChild(tr);
            });
        })
        .catch(() => {
            document.getElementById('status').textContent = 'Failed to fetch grid data.';
        });
}

fetchGrid();
setInterval(fetchGrid, 5000);