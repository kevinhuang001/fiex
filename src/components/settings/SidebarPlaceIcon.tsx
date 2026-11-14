import { t } from '@/i18n'
import {
  AppWindow,
  Clock,
  Download,
  FileText,
  Film,
  Folder,
  Home,
  Image,
  Monitor,
  Music,
  Trash2,
} from 'lucide-react'

export function SidebarPlaceIcon({ name }: { name: string }) {
  if (name === t('最近使用')) return <Clock size={14} />
  if (name === t('废纸篓') || name === t('回收站')) return <Trash2 size={14} />
  if (name === t('个人文件夹')) return <Home size={14} />
  if (name === t('桌面')) return <Monitor size={14} />
  if (name === t('下载')) return <Download size={14} />
  if (name === t('图片')) return <Image size={14} />
  if (name === t('音乐')) return <Music size={14} />
  if (name === t('视频')) return <Film size={14} />
  if (name === t('文稿')) return <FileText size={14} />
  if (name === t('应用程序')) return <AppWindow size={14} />
  return <Folder size={14} />
}
