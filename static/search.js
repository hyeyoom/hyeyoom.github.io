(function () {
  var input = document.getElementById('site-search-input');
  var results = document.getElementById('site-search-results');
  if (!input || !results) return;

  var indexPromise = null;
  var index = [];

  function loadIndex() {
    if (!indexPromise) {
      indexPromise = fetch('/search-index.json', { cache: 'force-cache' })
        .then(function (response) {
          if (!response.ok) throw new Error('search index load failed');
          return response.json();
        })
        .then(function (data) {
          index = Array.isArray(data) ? data : [];
          return index;
        })
        .catch(function () {
          index = [];
          return index;
        });
    }
    return indexPromise;
  }

  function normalize(text) {
    return String(text || '').toLowerCase().trim();
  }

  function queryTerms(query) {
    var seen = {};
    var terms = [];
    normalize(query)
      .split(/[^0-9a-z가-힣]+/i)
      .forEach(function (term) {
        addQueryTerm(terms, seen, term);
        if (/^[a-z]+s$/.test(term) && term.length > 3) {
          addQueryTerm(terms, seen, term.replace(/s$/, ''));
        }
        if (/^[a-z]+es$/.test(term) && term.length > 4) {
          addQueryTerm(terms, seen, term.replace(/es$/, ''));
        }
      });
    return terms;
  }

  function addQueryTerm(terms, seen, term) {
    if (term.length >= 2 && !seen[term]) {
      seen[term] = true;
      terms.push(term);
    }
  }

  function scoreItem(item, terms, rawQuery) {
    var title = normalize(item.title);
    var description = normalize(item.description);
    var excerpt = normalize(item.excerpt);
    var searchText = normalize(item.search_text);
    var keywords = (item.keywords || []).map(normalize);
    var indexedTerms = (item.terms || []).map(normalize);
    var score = 0;

    if (title.indexOf(rawQuery) !== -1) score += 80;
    if (description.indexOf(rawQuery) !== -1) score += 30;
    if (excerpt.indexOf(rawQuery) !== -1) score += 10;
    if (searchText.indexOf(rawQuery) !== -1) score += 6;

    terms.forEach(function (term) {
      if (title.indexOf(term) !== -1) score += 40;
      if (description.indexOf(term) !== -1) score += 15;
      if (excerpt.indexOf(term) !== -1) score += 4;
      if (searchText.indexOf(term) !== -1) score += 3;
      keywords.forEach(function (keyword, idx) {
        if (keyword === term) score += 24 - Math.min(idx, 12);
        else if (keyword.indexOf(term) !== -1 || term.indexOf(keyword) !== -1) score += 8;
      });
      indexedTerms.forEach(function (indexed) {
        if (indexed === term) score += 5;
        else if (indexed.indexOf(term) !== -1 || term.indexOf(indexed) !== -1) score += 2;
      });
    });

    return score;
  }

  function escapeHtml(text) {
    return String(text || '')
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }

  function render(items, query) {
    if (!query) {
      results.innerHTML = '';
      return;
    }

    if (items.length === 0) {
      results.innerHTML = '<p class="site-search-keywords">검색 결과가 없습니다.</p>';
      return;
    }

    results.innerHTML = items
      .slice(0, 8)
      .map(function (item) {
        var keywords = (item.keywords || []).slice(0, 5).join(', ');
        return (
          '<article class="site-search-result">' +
          '<a href="' + escapeHtml(item.url) + '">' + escapeHtml(item.title) + '</a>' +
          '<p>' + escapeHtml(item.description || item.excerpt || '') + '</p>' +
          (keywords ? '<div class="site-search-keywords">' + escapeHtml(keywords) + '</div>' : '') +
          '</article>'
        );
      })
      .join('');
  }

  function runSearch() {
    var rawQuery = normalize(input.value);
    var terms = queryTerms(rawQuery);
    if (terms.length === 0) {
      render([], '');
      return;
    }

    loadIndex().then(function () {
      var scored = index
        .map(function (item) {
          return { item: item, score: scoreItem(item, terms, rawQuery) };
        })
        .filter(function (entry) {
          return entry.score > 0;
        })
        .sort(function (a, b) {
          return b.score - a.score || a.item.title.localeCompare(b.item.title);
        })
        .map(function (entry) {
          return entry.item;
        });
      render(scored, rawQuery);
    });
  }

  input.addEventListener('focus', loadIndex, { once: true });
  input.addEventListener('input', runSearch);
})();
