// Getting started. Static text: nothing here reads the profile, so it works with no mods.
export default function Help({ onAdd = () => {} }) {
  return (
    <article className="help">
      <h2>Getting started</h2>

      <section>
        <h3>1. Getting your mods into Owl</h3>
        <p>
          Owl does not download or unpack mods. It works with folders on your machine, so you'll have to download and unpack mods yourself.
        </p>
        <ol>
          <li>Download the mod from its website.</li>
          <li>Extract the archive (.zip, .7z, .rar) to a folder. A mod you have not extracted cannot be added.</li>
          <li>
            Keep your mods somewhere permanent. Owl remembers where they are, so moving or deleting the folder breaks
            the mod on your list.
          </li>
        </ol>
        <p>Then show Owl where they are:</p>
        <ul>
          <li>
            <strong>+ Add a mod</strong> opens a folder chooser. Pick one mod's folder and it appears under
            <em> Available to add</em> as an individual mod.
          </li>
          <li>
            Pick a folder that holds many mods (your whole downloads folder, say) and Owl asks whether to show its mods
            in your list. Say yes and it becomes a source: each mod inside appears under <em>Available to add</em>, and
            new ones show up as you extract them.
          </li>
          <li>
            Open a mod there, choose the folders you want (or step through its installer, if it has one), then press
            <strong> Add to my mods</strong>. The <em>Author's notes</em> tab shows its readme first, and
            <em> Needs</em> lists what it requires.
          </li>
          <li>The ✕ next to a source or individual mod stops listing it. It does not delete anything.</li>
        </ul>
        <p>
          <button className="secondary" onClick={onAdd}>
            Add a mod
          </button>
        </p>
      </section>

      <section>
        <h3>2. Putting your mods in order</h3>
        <p>
          The numbered list on the left is your load order. Mods lower in the list load later and win wherever two mods
          change the same thing.
        </p>
        <ul>
          <li>Select a mod, then use <strong>Move up</strong> or <strong>Move down</strong>. There is no dragging.</li>
          <li>
            A mod's plugins stay together and in their own order. Owl orders mods, not individual plugins;
            <em> Review and finalize</em> shows the resulting plugin order.
          </li>
          <li>
            A mod that needs another mod's plugin must come after it. If one is in the wrong place, Owl flags it
            and offers <strong>Fix the order for me</strong>, which shows what it would move before doing it.
          </li>
        </ul>
      </section>

      <section>
        <h3>3. Applying it to OpenMW</h3>
        <p>
          Nothing changes in OpenMW until you apply. Go to <strong>Review and finalize</strong>, check the problems
          list (⚠ marks mods with a problem), then press <strong>Review changes</strong>. Owl shows exactly what it
          will change in your <code>openmw.cfg</code>, makes a backup copy (.bak), and only writes when you confirm.
          If the file changed since you reviewed, Owl refuses and asks you to review again.
        </p>
      </section>

      <section>
        <h3>4. Using the OpenMW Launcher too</h3>
        <p>
          Owl only manages the section of <code>openmw.cfg</code> between its own marker comments. Everything outside
          is left alone. The Launcher rewrites the file when it saves, and can put its own lines inside Owl's section.
          Owl keeps those lines and lists them as <em>kept</em> in the review. If you change things in the Launcher,
          review in Owl again before applying.
        </p>
      </section>

      <section>
        <h3>Not here yet</h3>
        <ul>
          <li>Downloading and extracting mods</li>
          <li>Restoring a backup from inside Owl (the .bak file sits next to <code>openmw.cfg</code>)</li>
          <li>Ordering individual plugins, and dragging to reorder</li>
        </ul>
      </section>

      <section>
        <h3>Why the name "Owl"?</h3>
        <p>The author of the app likes owls; that's all.</p>
      </section>
    </article>
  );
}
