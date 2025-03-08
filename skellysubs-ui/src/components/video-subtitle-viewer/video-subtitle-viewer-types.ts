import type {SubtitleVariant} from "../../store/slices/available-subtitles/available-subtitles-types";

export interface Subtitle {
    start: number;
    end: number;
    text: string[];
}

export interface SubtitleCue {
    id: string;
    name: string;
    variant: SubtitleVariant;
    language: string;
    content: string;
}
