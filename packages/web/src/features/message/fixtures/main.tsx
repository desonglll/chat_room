import { createRoot } from 'react-dom/client'
import { FixtureGallery } from './FixtureGallery'

const host = document.getElementById('fixtures')
if (host === null) throw new Error('fixtures: #fixtures is missing from index.html')

createRoot(host).render(<FixtureGallery />)
