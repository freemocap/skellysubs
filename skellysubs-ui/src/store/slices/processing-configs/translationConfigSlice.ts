import { createSlice, PayloadAction } from '@reduxjs/toolkit';
import type { LanguageConfig } from '../../../schemas/languageConfigSchemas';
import {RootState} from "../../AppStateStore";

interface TranslationConfigState {
    languageOptions: Record<string, LanguageConfig>;
    selectedTargetLanguages: string[];
}

const initialState: TranslationConfigState = {
    languageOptions: {},
    selectedTargetLanguages: [],
};

export const translationConfigSlice = createSlice({
    name: 'translationConfig',
    initialState,
    reducers: {
        setLanguageOptions: (state, action: PayloadAction<Record<string, LanguageConfig>>) => {
            state.languageOptions = action.payload;
        },
        toggleLanguage: (state, action: PayloadAction<string>) => {
            const code = action.payload;
            const index = state.selectedTargetLanguages.indexOf(code);
            if (index === -1) {
                state.selectedTargetLanguages.push(code);
            } else {
                state.selectedTargetLanguages.splice(index, 1);
            }
        },
        addCustomLanguage: (state, action: PayloadAction<LanguageConfig>) => {
            const lang = action.payload;
            const key = lang.language_code; // Use code instead of name
            state.languageOptions[key] = lang;
        },
        updateLanguageConfig: (
            state,
            action: PayloadAction<{ key: string; config: LanguageConfig }>
        ) => {
            const { key, config } = action.payload;
            state.languageOptions[key] = config;
        },
    },
});

export const {
    setLanguageOptions,
    toggleLanguage,
    addCustomLanguage,
    updateLanguageConfig,
} = translationConfigSlice.actions;

export const selectLanguageOptions = (state: RootState) =>
    state.translationConfig.languageOptions;

export const selectSelectedTargetLanguages = (state: RootState) =>
    state.translationConfig.selectedTargetLanguages;