'use client'

export default function Home() {
    return (
        <main className="w-full max-w-[800px] flex flex-col gap-6">
            <header className="header flex justify-between items-center border border-border p-4 bg-black/50 backdrop-blur">
                <div>
                    <h1 className="text-xl font-bold tracking-[0.2em] uppercase">W9 Links</h1>
                    <p className="text-xs text-gray-500 uppercase tracking-widest mt-1">Short Link Generator</p>
                </div>
            </header>

            <section className="box">
                <h2 className="text-lg uppercase tracking-widest mb-4 border-b border-border pb-2">Create Link</h2>
                <form className="flex flex-col gap-4">
                    <div>
                        <label className="block text-xs uppercase tracking-widest text-gray-500 mb-1">Target URL</label>
                        <input type="text" className="w-full bg-black border border-border p-3 text-white focus:border-white focus:outline-none" placeholder="https://example.com" />
                    </div>
                    <div>
                        <label className="block text-xs uppercase tracking-widest text-gray-500 mb-1">Custom Alias (Optional)</label>
                        <div className="flex items-center">
                            <span className="text-gray-500 bg-border/20 p-3 border border-r-0 border-border">w9.nu/s/</span>
                            <input type="text" className="w-full bg-black border border-border p-3 text-white focus:border-white focus:outline-none" placeholder="my-link" />
                        </div>
                    </div>
                    <button type="button" className="button accent self-start mt-2">Shorten</button>
                </form>
            </section>
        </main>
    )
}
