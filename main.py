import asyncio
import os
from dataclasses import dataclass
from urllib.parse import urlparse

import aiohttp
from aiohttp import ClientSession
from loguru import logger
from tqdm import tqdm


@dataclass
class Img:
    url: str
    filepath: str


async def dl_url(session: ClientSession, img: Img):
    has_file = os.path.exists(img.filepath)
    if has_file:  # skip when already downloaded
        logger.info(f'skipping {img}')
        return

    async with session.get(img.url) as resp:
        if resp.status != 200:
            logger.error(f'{img.url}: {resp}')
            return

        content = await resp.read()
        with open(img.filepath, 'wb') as f:
            f.write(content)


async def batch_dl(session: ClientSession, imgs: list[Img]):
    tasks = [asyncio.ensure_future(dl_url(session, i)) for i in imgs]
    await asyncio.gather(*tasks)


def mk_filepath(url: str):
    filename = os.path.basename(urlparse(url).path)
    return os.path.join('out', filename)


async def main():
    batch_file = 'urls.txt'
    batch_size = 10

    with open(batch_file) as f:
        urls = f.readlines()

    async with aiohttp.ClientSession() as session:
        batches = [urls[i:i + batch_size] for i in range(0, len(urls), batch_size)]
        for batch_url in tqdm(batches):
            imgs = [Img(url=url, filepath=mk_filepath(url)) for url in batch_url]
            await batch_dl(session, imgs)


if __name__ == '__main__':
    asyncio.run(main())
