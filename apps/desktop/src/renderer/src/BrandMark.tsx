import type { ImgHTMLAttributes } from 'react'
import icon from './assets/coaxi-icon.png'

/** Customer-approved artwork, shared by every product lockup. */
export function BrandMark({ style, ...props }: Omit<ImgHTMLAttributes<HTMLImageElement>, 'src' | 'alt'>): React.JSX.Element {
  return <img width={24} height={24} {...props} src={icon} alt="" aria-hidden="true" draggable={false}
    data-brand-mark="coaxi" data-brand-layout="flat" style={{ objectFit: 'contain', ...style }} />
}
