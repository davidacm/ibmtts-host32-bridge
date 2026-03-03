#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(dead_code)]

use std::os::raw::{ c_int, c_void};

// Enums replicated from eci.h
pub type Boolean = c_int;
pub type ECIHand = *mut c_void;
pub type ECIInputText = *const c_void;
pub type ECIDictHand = *mut c_void;

#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ECIParam {
    eciSynthMode = 0,
    eciInputType,
    eciTextMode,
    eciDictionary,
    eciSampleRate = 5,
    eciWantPhonemeIndices = 7,
    eciRealWorldUnits,
    eciLanguageDialect,
    eciNumberMode,
    eciWantWordIndex = 12,
    eciNumDeviceBlocks,
    eciSizeDeviceBlocks,
    eciNumPrerollDeviceBlocks,
    eciSizePrerollDeviceBlocks,
    eciNumParams,
}

#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ECIVoiceParam {
    eciGender = 0,
    eciHeadSize,
    eciPitchBaseline,
    eciPitchFluctuation,
    eciRoughness,
    eciBreathiness,
    eciSpeed,
    eciVolume,
    eciNumVoiceParams,
}

#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ECIDictVolume {
    eciMainDict = 0,
    eciRootDict = 1,
    eciAbbvDict = 2,
    eciMainDictExt = 3,
}

#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ECIDictError {
    DictNoError = 0,
    DictFileNotFound = 1,
    DictOutOfMemory = 2,
    DictInternalError = 3,
    DictNoEntry = 4,
    DictErrLookUpKey = 5,
    DictAccessError = 6,
}

#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ECILanguageDialect {
    NODEFINEDCODESET = 0x00000000,
    eciGeneralAmericanEnglish = 0x00010000,
    eciBritishEnglish = 0x00010001,
    eciCastilianSpanish = 0x00020000,
    eciMexicanSpanish = 0x00020001,
    eciStandardFrench = 0x00030000,
    eciCanadianFrench = 0x00030001,
    eciStandardGerman = 0x00040000,
    eciStandardItalian = 0x00050000,
    eciMandarinChinese = 0x00060000,
    eciMandarinChinesePinYin = 0x00060100,
    eciMandarinChineseUCS = 0x00060800,
    eciTaiwaneseMandarin = 0x00060001,
    eciTaiwaneseMandarinZhuYin = 0x00060101,
    eciTaiwaneseMandarinPinYin = 0x00060201,
    eciTaiwaneseMandarinUCS = 0x00060801,
    eciBrazilianPortuguese = 0x00070000,
    eciStandardJapanese = 0x00080000,
    eciStandardJapaneseUCS = 0x00080800,
    eciStandardFinnish = 0x00090000,
    eciStandardKorean = 0x000A0000,
}

#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ECIMessage {
    eciWaveformBuffer = 0,
    eciPhonemeBuffer,
    eciIndexReply,
    eciPhonemeIndexReply,
    eciWordIndexReply,
}

#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ECICallbackReturn {
    eciDataNotProcessed = 0,
    eciDataProcessed,
    eciDataAbort,
}


pub type EciCallback = unsafe extern "system" fn(
    hEngine: ECIHand,
    msg: u32,
    lparam: i32,
    pData: *mut c_void,
) -> ECICallbackReturn;
