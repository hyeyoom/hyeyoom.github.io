import hashlib
import json
import os.path
import time
import shutil
from datetime import datetime, timedelta
from urllib.parse import unquote, quote

import markdown
from bs4 import BeautifulSoup
from jinja2 import Environment, FileSystemLoader, Template
from lxml import html


class WikiManager:
    def __init__(self, store_path: str = './meta.json'):
        try:
            with open(store_path, 'r', encoding='utf-8') as file:
                self.meta = json.load(file)
        except FileNotFoundError:
            self.meta = {}
            with open(store_path, 'w', encoding='utf-8') as file:
                json.dump(self.meta, file, ensure_ascii=False)

    def flush(self, store_path: str = './meta.json'):
        with open(store_path, 'w', encoding='utf-8') as file:
            json.dump(self.meta, file, ensure_ascii=False)

    def get_meta(self, key: str):
        return self.meta.get(key)

    def get_document_created_time(self, document_name: str) -> float | None:
        document_meta = self.meta.get(document_name)
        if document_meta is None:
            return None
        return document_meta.get('created_time')

    def get_document_modified_time(self, document_name: str) -> float | None:
        document_meta = self.meta.get(document_name)
        if document_meta is None:
            return None
        return document_meta.get('modified_time')

    def set_document_created_time(self, document_name: str, created_timestamp: float):
        if document_name not in self.meta:
            self.meta[document_name] = {}
        document_meta = self.meta[document_name]
        if document_meta.get('created_time') is None:
            document_meta['created_time'] = created_timestamp

    def set_document_modified_time(self, document_name: str, modified_timestamp: float):
        if document_name not in self.meta:
            self.meta[document_name] = {}
        document_meta = self.meta[document_name]
        document_meta['modified_time'] = modified_timestamp

    def set_document_hash(self, document_name: str, document_hash: str):
        if document_name not in self.meta:
            self.meta[document_name] = {}
        document_meta = self.meta[document_name]
        document_meta['document_hash'] = document_hash

    def get_document_hash(self, document_name: str) -> str | None:
        document_meta = self.meta.get(document_name)
        if document_meta is None:
            return None
        return document_meta.get('document_hash')


class WikiDocument:
    __TIME_DELTA_FOR_KST = timedelta(hours=9)
    _hasher = hashlib.sha256()

    def __init__(self, file_path: str, template: Template, wiki_manager: WikiManager):
        self.document_name = self._extract_filename_without_extension(file_path)
        self.raw_content = self._read_file(file_path)
        self.file_path = file_path
        self.template = template
        self.wiki_manager = wiki_manager

        current_hash, previous_hash, should_update = self.should_update_determine(file_path)
        self.should_update = should_update

        if should_update:
            self.document_hash = current_hash
            self.wiki_manager.set_document_hash(self.document_name, self.document_hash)
            modified_timestamp = os.path.getmtime(file_path)
            self.modified_datetime = self._convert_timestamp_to_datetime(modified_timestamp)
            self.wiki_manager.set_document_modified_time(self.document_name, modified_timestamp)
        else:
            self.document_hash = previous_hash

        modified_timestamp = self.wiki_manager.get_document_modified_time(self.document_name)
        self.modified_datetime = self._convert_timestamp_to_datetime(modified_timestamp)

        if self.wiki_manager.get_document_created_time(self.document_name) is None:
            created_timestamp = os.path.getctime(file_path)
            self.created_datetime = self._convert_timestamp_to_datetime(created_timestamp)
            self.wiki_manager.set_document_created_time(self.document_name, created_timestamp)
        else:
            created_timestamp = self.wiki_manager.get_document_created_time(self.document_name)
            self.created_datetime = self._convert_timestamp_to_datetime(created_timestamp)

    def should_update_determine(self, file_path):
        current_hash = self._calculate_file_hash(file_path)
        previous_hash = self.wiki_manager.get_document_hash(self.document_name)
        should_update = current_hash != previous_hash
        return current_hash, previous_hash, should_update

    def get_last_modified_datetime(self) -> datetime:
        if self.modified_datetime is None:
            return self.created_datetime
        return self.modified_datetime

    def _calculate_file_hash(self, file_path: str, block_size=65536):
        with open(file_path, 'rb') as file:
            for block in iter(lambda: file.read(block_size), b''):
                self._hasher.update(block)

        return self._hasher.hexdigest()

    def _read_file(self, filepath: str) -> str:
        with open(filepath, 'r', encoding='utf-8') as file:
            return file.read()

    def _extract_filename_without_extension(self, file_path):
        filename = self._extract_filename(file_path)
        filename_without_ext, _ = os.path.splitext(filename)
        return filename_without_ext

    def _extract_filename(self, file_path):
        return os.path.basename(file_path)

    def _convert_timestamp_to_datetime(self, timestamp: float) -> datetime:
        dt = datetime.utcfromtimestamp(timestamp)
        localized_dt = dt + self.__TIME_DELTA_FOR_KST
        return localized_dt

    def __repr__(self):
        return f'WikiDocument({self.document_name} {self.document_hash})'

    def _markdown_to_html(self, markdown_text: str) -> str:
        html_content = markdown.markdown(markdown_text,
                                         extensions=['tables', 'markdown_checklist.extension', 'fenced_code'])
        html_element = html.fromstring(html_content)

        return html.tostring(html_element, pretty_print=True, encoding='unicode')

    @staticmethod
    def _post_process(html_document: str) -> str:
        soup = BeautifulSoup(html_document, 'html.parser')
        for a_tag in soup.find_all('a', href=True):
            origin = a_tag['href']
            if (origin.startswith('http')):
                continue
            if (origin.startswith('%')):
                origin = unquote(origin)
            if origin[0].isalpha() or origin[0] == '.' or origin[0].isdigit() or (
                    ord('가') <= ord(origin[0]) <= ord('힣')):
                a_tag['href'] = f"{origin}.html"
        return str(soup)

    def render_in_html(self) -> str:
        rendered_markdown = self._markdown_to_html(self.raw_content)
        postProcessed = self._post_process(rendered_markdown)
        return self.template.render(
            title=self.document_name,
            body=postProcessed
        )


def get_file_list_by_ext(directory_path: str, ext: str) -> list:
    filenames = [f for f in os.listdir(directory_path) if os.path.isfile(os.path.join(directory_path, f))]
    return list(filter(lambda x: x.endswith(ext), filenames))


def render_html_in_file():
    html_output = doc.render_in_html()
    html_output_path = os.path.join('./docs', f"{doc.document_name}.html")
    with open(html_output_path, 'w', encoding='utf-8') as file:
        file.write(html_output)
        file.flush()


class SitemapGenerator:
    __base_url = 'https://wiki.chiho.one/'

    __sitemap_template = """<?xml version="1.0" encoding="UTF-8"?>
            <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
            {0}
            </urlset>
            """

    __url_template = """<url>
                    <loc>{0}</loc>
                    <changefreq>weekly</changefreq>
                    <lastmod>{1}</lastmod>
                    <priority>0.8</priority>
                </url>"""

    def __init__(self, docs: list[WikiDocument]):
        self.docs = docs

    @staticmethod
    def __save_file(filepath: str, content: str) -> None:
        with open(filepath, 'w') as file:
            file.write(content)
            file.flush()

    def generate(self):
        url_elements = []
        for doc in docs:
            lastmod = str(doc.modified_datetime).replace(' ', 'T') + '+09:00'
            full_url = self.__base_url + quote(f"{doc.document_name}.html")
            url = self.__url_template.format(full_url.replace(" ", ""), lastmod)
            url_elements.append(url)
        sitemap = self.__sitemap_template.format('\n'.join(url_elements))
        self.__save_file('./docs/sitemap.xml', sitemap)


def copy_assets():
    shutil.copytree('./wiki/assets', './docs/assets', dirs_exist_ok=True)

if __name__ == '__main__':
    start_time = time.perf_counter()

    template_loader = FileSystemLoader(searchpath='./wiki_template')
    env = Environment(loader=template_loader)
    template = env.get_template('document_template.html')

    wm = WikiManager()

    filenames = get_file_list_by_ext('./wiki', '.md')
    targets = list(map(lambda x: os.path.join('./wiki', x), filenames))

    count_of_changed = 0
    docs = []
    repainted = False
    for target in targets:
        doc = WikiDocument(target, template, wm)
        docs.append(doc)
        if doc.should_update:
            count_of_changed += 1
            render_html_in_file()
            repainted = True

    wm.flush()

    if repainted:
        print(f"Repainted. docs: {len(docs)} files. changed: {count_of_changed} files.")
        copy_assets()
        SitemapGenerator(docs).generate()
    else:
        print(f"Nothing to update. docs: {len(docs)} files. changed: {count_of_changed} files.")

    end_time = time.perf_counter()
    elapsed_time = end_time - start_time
    print(f"Elapsed Time: {elapsed_time} seconds")
